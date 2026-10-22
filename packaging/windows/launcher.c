/* EN-US: A system-only bootstrap establishes runtime paths before GTK DLL loading.
 * PT-BR: Um iniciador que só usa o sistema define caminhos antes de carregar DLLs GTK. */
#define _WIN32_WINNT 0x0602
#include <windows.h>
#include <shlobj.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <stdint.h>

#define CAP 32768
#ifdef PAPO_GUI
static int interactive_error = 1;
#else
static int interactive_error = 0;
#endif
static void die(const wchar_t *message) {
    fwprintf(stderr, L"Papo: %ls (Windows error %lu)\n", message, GetLastError());
    if (interactive_error) MessageBoxW(NULL, message, L"Papo", MB_OK | MB_ICONERROR);
    exit(1);
}
static void path(wchar_t *out, const wchar_t *root, const wchar_t *suffix) {
    if (swprintf(out, CAP, L"%ls\\%ls", root, suffix) < 0) die(L"Runtime path too long");
}
static void environment(const wchar_t *name, const wchar_t *value) {
    if (!SetEnvironmentVariableW(name, value) && (value || GetLastError() != ERROR_ENVVAR_NOT_FOUND))
        die(L"Cannot configure runtime environment");
}
static void env_path(const wchar_t *name, const wchar_t *root, const wchar_t *suffix) {
    wchar_t value[CAP]; path(value, root, suffix);
    environment(name, value);
}
static int file(const wchar_t *name) {
    DWORD attributes = GetFileAttributesW(name);
    return attributes != INVALID_FILE_ATTRIBUTES && !(attributes & FILE_ATTRIBUTE_DIRECTORY);
}
static void character(wchar_t *buffer, size_t *used, wchar_t value) {
    if (*used + 1 >= CAP) die(L"Command line too long");
    buffer[(*used)++] = value; buffer[*used] = 0;
}
/* EN-US: Match CommandLineToArgvW quoting, including trailing backslashes.
 * PT-BR: Reproduz as aspas de CommandLineToArgvW, incluindo barras finais. */
static void argument(wchar_t *buffer, size_t *used, const wchar_t *value) {
    if (*used) character(buffer, used, L' ');
    character(buffer, used, L'"');
    while (*value) {
        size_t slashes = 0;
        while (*value == L'\\') { slashes++; value++; }
        size_t count = (*value == L'"' || !*value) ? slashes * 2 : slashes;
        while (count--) character(buffer, used, L'\\');
        if (*value == L'"') character(buffer, used, L'\\');
        if (*value) character(buffer, used, *value++);
    }
    character(buffer, used, L'"');
}
static int launch(int argc, wchar_t **argv) {
    int diagnostics = argc > 1 && wcscmp(argv[1], L"--activate") != 0;
    if (diagnostics) {
        interactive_error = 0;
        if (!GetStdHandle(STD_OUTPUT_HANDLE)) AttachConsole(ATTACH_PARENT_PROCESS);
    }
    if (argc > 1 && wcscmp(argv[1], L"--diagnostics") == 0) {
        if (!GetConsoleWindow() && !GetStdHandle(STD_ERROR_HANDLE)) AllocConsole();
        for (int i = 1; i < argc - 1; i++) argv[i] = argv[i + 1];
        argc--;
    }
    /* EN-US: Missing bundled imports must fail instead of opening a blocking loader dialog.
     * PT-BR: Imports ausentes devem falhar sem abrir uma janela bloqueante do carregador. */
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    wchar_t own[CAP], root[CAP], bin[CAP], target[CAP], command[CAP] = {0}, system[CAP];
    DWORD length = GetModuleFileNameW(NULL, own, CAP);
    if (!length || length >= CAP) die(L"Cannot locate launcher");
    wchar_t *separator = wcsrchr(own, L'\\');
    if (!separator) die(L"Invalid launcher location");
    *separator = 0;
    DWORD override = GetEnvironmentVariableW(L"PAPO_RUNTIME_DIR", root, CAP);
    if (override >= CAP) die(L"Runtime override too long");
    if (!override) path(root, own, L"runtime");
    wchar_t absolute[CAP];
    DWORD absolute_length = GetFullPathNameW(root, CAP, absolute, NULL);
    if (!absolute_length || absolute_length >= CAP) die(L"Invalid runtime path");
    wcscpy(root, absolute);
    path(bin, root, L"bin");
    int shell = argc > 1 && (wcscmp(argv[1], L"--register-desktop") == 0 || wcscmp(argv[1], L"--unregister-desktop") == 0 || wcscmp(argv[1], L"--desktop-info") == 0 || wcscmp(argv[1], L"--test-toast") == 0 || wcscmp(argv[1], L"--clear-test-toast") == 0 || wcscmp(argv[1], L"--test-toast-info") == 0);
    path(target, root, shell ? L"bin\\papo-shell.exe" : L"bin\\papo-gtk.exe");
    if (!file(target)) die(L"Bundled application is missing; extract the entire ZIP");
    wchar_t marker[CAP]; path(marker, root, L"runtime.id");
    HANDLE identity = CreateFileW(marker, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
    char id[64]; DWORD read = 0;
    if (identity == INVALID_HANDLE_VALUE || !ReadFile(identity, id, 64, &read, NULL) || read != 64)
        die(L"Runtime identity is missing");
    CloseHandle(identity);
    uint64_t hash = UINT64_C(14695981039346656037);
    for (const wchar_t *p = root; *p; p++) { hash ^= (uint16_t)*p; hash *= UINT64_C(1099511628211); }
    for (size_t i = 0; i < 64; i++) { hash ^= (unsigned char)id[i]; hash *= UINT64_C(1099511628211); }
    wchar_t cache_base[CAP], cache[CAP];
    DWORD local_length = GetEnvironmentVariableW(L"LOCALAPPDATA", cache_base, CAP);
    if (local_length >= CAP) die(L"Local cache directory too long");
    if (!local_length &&
        FAILED(SHGetFolderPathW(NULL, CSIDL_LOCAL_APPDATA, NULL, 0, cache_base))) die(L"Local cache directory unavailable");
    if (swprintf(cache, CAP, L"%ls\\papo-gtk\\runtime\\%016llx", cache_base, (unsigned long long)hash) < 0)
        die(L"Cache path too long");
    int created = SHCreateDirectoryExW(NULL, cache, NULL);
    if (created != ERROR_SUCCESS && created != ERROR_ALREADY_EXISTS && created != ERROR_FILE_EXISTS)
        die(L"Cannot create runtime cache");
    DWORD system_length = GetSystemDirectoryW(system, CAP);
    if (!system_length || system_length >= CAP) die(L"System directory unavailable");
    wchar_t search[CAP];
    if (swprintf(search, CAP, L"%ls;%ls", bin, system) < 0) die(L"DLL path too long");
    environment(L"PATH", search);
    environment(L"PAPO_RUNTIME_DIR", root);
    environment(L"PYTHONPATH", NULL);
    environment(L"PYTHONNOUSERSITE", L"1");
    environment(L"PYTHONSAFEPATH", L"1");
    environment(L"PYTHONDONTWRITEBYTECODE", L"1");
    if (!GetEnvironmentVariableW(L"PAPO_PYTHON", NULL, 0)) environment(L"PYTHONHOME", root);
    env_path(L"GI_TYPELIB_PATH", root, L"lib\\girepository-1.0");
    env_path(L"GSETTINGS_SCHEMA_DIR", root, L"share\\glib-2.0\\schemas");
    env_path(L"GIO_MODULE_DIR", root, L"lib\\gio\\modules");
    environment(L"GIO_EXTRA_MODULES", NULL);
    env_path(L"GTK_PATH", root, L"lib\\gtk-4.0");
    env_path(L"XDG_DATA_DIRS", root, L"share");
    env_path(L"FONTCONFIG_PATH", root, L"etc\\fonts");
    env_path(L"FONTCONFIG_FILE", root, L"etc\\fonts\\fonts.conf");
    env_path(L"GST_PLUGIN_SYSTEM_PATH_1_0", root, L"lib\\gstreamer-1.0");
    environment(L"GST_PLUGIN_SYSTEM_PATH", NULL);
    environment(L"GST_PLUGIN_PATH", NULL);
    environment(L"GST_PLUGIN_PATH_1_0", NULL);
    env_path(L"GST_PLUGIN_SCANNER_1_0", root, L"libexec\\gstreamer-1.0\\gst-plugin-scanner.exe");
    env_path(L"GST_REGISTRY_1_0", cache, L"registry.bin");
    environment(L"GST_REGISTRY", NULL);
    environment(L"XDG_CACHE_HOME", cache);
    wchar_t pattern[CAP]; path(pattern, root, L"lib\\gdk-pixbuf-2.0\\*");
    WIN32_FIND_DATAW entry; HANDLE found = FindFirstFileW(pattern, &entry); int loader = 0;
    if (found != INVALID_HANDLE_VALUE) {
        do {
            if (entry.cFileName[0] == L'.') continue;
            wchar_t directory[CAP], suffix[CAP];
            if (swprintf(suffix, CAP, L"lib\\gdk-pixbuf-2.0\\%ls", entry.cFileName) < 0) die(L"Loader path too long");
            path(directory, root, suffix); path(marker, directory, L"loaders.cache");
            if (file(marker)) {
                environment(L"GDK_PIXBUF_MODULE_FILE", marker);
                env_path(L"GDK_PIXBUF_MODULEDIR", directory, L"loaders"); loader = 1; break;
            }
        } while (FindNextFileW(found, &entry));
        FindClose(found);
    }
    if (!loader) die(L"Bundled image loader cache is missing");
    if (!SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_USER_DIRS) || !SetDllDirectoryW(L""))
        die(L"Cannot restrict DLL search");
    if (!AddDllDirectory(bin)) die(L"Cannot register runtime DLL directory");
    size_t used = 0; argument(command, &used, target);
    for (int i = 1; i < argc; i++) argument(command, &used, argv[i]);
    STARTUPINFOW startup = {0}; startup.cb = sizeof(startup);
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    startup.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
    startup.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    PROCESS_INFORMATION child = {0};
    /* EN-US: The child's own executable directory contains its complete DLL closure.
     * PT-BR: O diretório do executável filho contém todas as suas dependências DLL. */
    if (!CreateProcessW(target, command, NULL, NULL, TRUE, diagnostics ? 0 : CREATE_NO_WINDOW, NULL, bin, &startup, &child))
        die(L"Cannot start bundled application");
    CloseHandle(child.hThread);
    if (WaitForSingleObject(child.hProcess, INFINITE) != WAIT_OBJECT_0) die(L"Cannot wait for application");
    DWORD status = 1;
    if (!GetExitCodeProcess(child.hProcess, &status)) die(L"Cannot read application exit status");
    CloseHandle(child.hProcess);
    return (int)status;
}

/* EN-US: Explorer gets a GUI entry point; Papo.com deliberately retains console diagnostics.
 * PT-BR: Explorer usa entrada gráfica; Papo.com mantém diagnósticos no console de propósito. */
#ifdef PAPO_GUI
int WINAPI wWinMain(HINSTANCE instance, HINSTANCE previous, PWSTR tail, int show) {
    (void)instance; (void)previous; (void)tail; (void)show;
    int argc; wchar_t **argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (!argv) die(L"Cannot parse Windows command line");
    int status = launch(argc, argv); LocalFree(argv); return status;
}
#else
int wmain(int argc, wchar_t **argv) { return launch(argc, argv); }
#endif
