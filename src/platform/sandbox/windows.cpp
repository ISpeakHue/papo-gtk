// EN-US: Native AppContainer boundary. Jobs own lifetime/quotas, never filesystem/network policy.
// PT-BR: Fronteira AppContainer nativa. Jobs controlam vida/limites, nunca a política de arquivos/rede.
#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0A00
#include <windows.h>
#include <userenv.h>
#include <sddl.h>
#include <aclapi.h>
#include <string>
#include <vector>
#include <mutex>
#include <set>
#include <algorithm>
#include <cstring>
#include <memory>

static bool own_profile(const wchar_t* name);
static void cleanup_stale();
namespace {
struct Failure { DWORD code; };
void require(bool ok) { if (!ok) throw Failure{GetLastError() ? GetLastError() : ERROR_ACCESS_DENIED}; }
void status(DWORD code) { if (code) throw Failure{code}; }
void hresult(HRESULT code) { if (FAILED(code)) throw Failure{static_cast<DWORD>(code)}; }
struct Handle {
    HANDLE value = nullptr;
    Handle() = default;
    explicit Handle(HANDLE h) : value(h) {}
    ~Handle() { reset(); }
    void reset(HANDLE h = nullptr) { if (value && value != INVALID_HANDLE_VALUE) CloseHandle(value); value = h; }
    Handle(const Handle&) = delete; Handle& operator=(const Handle&) = delete;
};
struct Local { void* value = nullptr; ~Local() { if (value) LocalFree(value); } };
struct Capability {
    PSID sid = nullptr;
    Capability() {
        PSID *groups = nullptr, *caps = nullptr; DWORD ng = 0, nc = 0;
        using Derive=BOOL (WINAPI*)(LPCWSTR,PSID**,DWORD*,PSID**,DWORD*);
        auto address=GetProcAddress(GetModuleHandleW(L"kernelbase.dll"),"DeriveCapabilitySidsFromName"); require(address!=nullptr);
        Derive derive=nullptr; static_assert(sizeof(derive)==sizeof(address)); std::memcpy(&derive,&address,sizeof(derive));
        require(derive(L"br.com.papo.gtk.media.runtime.read", &groups, &ng, &caps, &nc));
        if (nc == 1) { sid = caps[0]; }
        for (DWORD i=0; i<ng; ++i) LocalFree(groups[i]);
        for (DWORD i=0; i<nc; ++i) if (caps[i] != sid) LocalFree(caps[i]);
        LocalFree(groups); LocalFree(caps); require(sid != nullptr);
    }
    ~Capability() { if (sid) LocalFree(sid); }
};
std::wstring quote(const std::wstring& arg) {
    std::wstring out=L"\""; size_t slashes=0;
    for (wchar_t c:arg) {
        if (c==L'\\') { ++slashes; continue; }
        out.append(slashes*(c==L'"'?2:1),L'\\'); slashes=0;
        if (c==L'"') out+=L'\\';
        out+=c;
    }
    out.append(slashes*2,L'\\'); return out+L'"';
}
void grant(const std::wstring& path, PSID sid, DWORD rights, DWORD inheritance, bool protect=false, ACCESS_MODE mode=GRANT_ACCESS) {
    PACL old=nullptr; Local descriptor;
    status(GetNamedSecurityInfoW(path.c_str(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,nullptr,nullptr,&old,nullptr,&descriptor.value));
    EXPLICIT_ACCESSW access{}; access.grfAccessPermissions=rights; access.grfAccessMode=mode;
    access.grfInheritance=inheritance; access.Trustee.TrusteeForm=TRUSTEE_IS_SID;
    access.Trustee.TrusteeType=TRUSTEE_IS_UNKNOWN; access.Trustee.ptstrName=static_cast<LPWSTR>(sid);
    PACL merged=nullptr; status(SetEntriesInAclW(1,&access,old,&merged)); Local acl; acl.value=merged;
    status(SetNamedSecurityInfoW(const_cast<LPWSTR>(path.c_str()),SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION|(protect?PROTECTED_DACL_SECURITY_INFORMATION:0),nullptr,nullptr,merged,nullptr));
}
constexpr DWORD write_rights=FILE_WRITE_DATA|FILE_APPEND_DATA|FILE_WRITE_EA|FILE_WRITE_ATTRIBUTES|FILE_DELETE_CHILD|DELETE|WRITE_DAC|WRITE_OWNER;
void runtime_access(const std::wstring& root, PSID sid) {
    // EN-US: A single private capability grants only runtime reads, shared by distinct per-job identities.
    // PT-BR: Uma capacidade privada permite só ler o runtime, compartilhada por identidades distintas por job.
    static std::mutex mutex; static std::set<std::wstring> ready;
    std::lock_guard<std::mutex> lock(mutex);
    if (ready.count(root)) return;
    require(GetFileAttributesW((root+L"\\bin\\python.exe").c_str())!=INVALID_FILE_ATTRIBUTES);
    PACL acl=nullptr; Local descriptor;
    status(GetNamedSecurityInfoW(root.c_str(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,nullptr,nullptr,&acl,nullptr,&descriptor.value));
    bool readable=false,denied=false;
    if(acl) for(DWORD i=0;i<acl->AceCount;++i) {
        void* raw=nullptr; require(GetAce(acl,i,&raw)); auto header=static_cast<ACE_HEADER*>(raw);
        if((header->AceFlags&(OBJECT_INHERIT_ACE|CONTAINER_INHERIT_ACE))!=(OBJECT_INHERIT_ACE|CONTAINER_INHERIT_ACE)||(header->AceFlags&INHERIT_ONLY_ACE)) continue;
        if(header->AceType==ACCESS_ALLOWED_ACE_TYPE) {
            auto ace=static_cast<ACCESS_ALLOWED_ACE*>(raw);
            if(EqualSid(&ace->SidStart,sid)&&(ace->Mask&(FILE_GENERIC_READ|FILE_GENERIC_EXECUTE))==(FILE_GENERIC_READ|FILE_GENERIC_EXECUTE)) readable=true;
        } else if(header->AceType==ACCESS_DENIED_ACE_TYPE) {
            auto ace=static_cast<ACCESS_DENIED_ACE*>(raw);
            if(EqualSid(&ace->SidStart,sid)&&(ace->Mask&write_rights)==write_rights) denied=true;
        }
    }
    // EN-US: Existing installer grants need no WRITE_DAC or repeat subtree propagation at startup.
    // PT-BR: Permissões já dadas pelo instalador dispensam WRITE_DAC e propagação repetida ao iniciar.
    if(!denied) grant(root,sid,write_rights,SUB_CONTAINERS_AND_OBJECTS_INHERIT,false,DENY_ACCESS);
    if(!readable) grant(root,sid,FILE_GENERIC_READ|FILE_GENERIC_EXECUTE,SUB_CONTAINERS_AND_OBJECTS_INHERIT);
    ready.insert(root);
}

struct Profile {
    std::wstring name, folder; PSID sid=nullptr; bool created=false;
    explicit Profile(const wchar_t* value):name(value) {
        require(name.size()>11 && name.size()<64 && name.rfind(L"Papo.Media.",0)==0);
        hresult(CreateAppContainerProfile(name.c_str(),L"Papo isolated media",L"Disposable offline decoder",nullptr,0,&sid)); created=true;
    }
    void locate() {
        LPWSTR text=nullptr; require(ConvertSidToStringSidW(sid,&text)); Local string; string.value=text;
        PWSTR path=nullptr; hresult(GetAppContainerFolderPath(text,&path)); folder=path; CoTaskMemFree(path);
    }
    bool cleanup() {
        if (!created) return true;
        for (int i=0;i<20;++i) { if (SUCCEEDED(DeleteAppContainerProfile(name.c_str()))) { created=false; return true; } Sleep(25); }
        return false;
    }
    ~Profile() { cleanup(); if(sid) FreeSid(sid); }
};
std::wstring input_folder(const std::wstring& name) {
    wchar_t temp[32768]; const DWORD size=GetTempPathW(32768,temp); require(size>0&&size<32768);
    return std::wstring(temp)+L"papo-input-"+name;
}
struct Input {
    std::wstring folder; std::vector<std::wstring> files; Handle directory,plugins;
    std::vector<std::unique_ptr<Handle>> sealed;
    explicit Input(const std::wstring& name):folder(input_folder(name)) { require(CreateDirectoryW(folder.c_str(),nullptr)); }
    void lock() { directory.reset(CreateFileW(folder.c_str(),FILE_READ_ATTRIBUTES,FILE_SHARE_READ|FILE_SHARE_WRITE,nullptr,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS,nullptr)); require(directory.value!=INVALID_HANDLE_VALUE); }
    void seal(const std::wstring& path) { auto handle=std::make_unique<Handle>();
        handle->reset(CreateFileW(path.c_str(),GENERIC_READ,FILE_SHARE_READ,nullptr,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,nullptr)); require(handle->value!=INVALID_HANDLE_VALUE); sealed.push_back(std::move(handle)); }
    void lock_plugins(const std::wstring& path) { plugins.reset(CreateFileW(path.c_str(),FILE_READ_ATTRIBUTES,FILE_SHARE_READ|FILE_SHARE_WRITE,nullptr,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS,nullptr)); require(plugins.value!=INVALID_HANDLE_VALUE); }
    std::wstring file(const std::wstring& suffix) { const auto path=folder+L"\\"+suffix; files.push_back(path); return path; }
    bool cleanup() {
        sealed.clear(); plugins.reset(); directory.reset();
        for(int attempt=0;attempt<40;++attempt) {
            for(const auto& file:files) DeleteFileW(file.c_str());
            RemoveDirectoryW((folder+L"\\plugins").c_str());
            if(RemoveDirectoryW(folder.c_str())||GetLastError()==ERROR_PATH_NOT_FOUND||GetLastError()==ERROR_FILE_NOT_FOUND) return true;
            Sleep(25);
        }
        return false;
    }
    ~Input() { cleanup(); }
};

struct Attributes {
    std::vector<unsigned char> memory; LPPROC_THREAD_ATTRIBUTE_LIST list=nullptr;
    Attributes() { SIZE_T size=0; InitializeProcThreadAttributeList(nullptr,2,0,&size); memory.resize(size);
        list=reinterpret_cast<LPPROC_THREAD_ATTRIBUTE_LIST>(memory.data()); require(InitializeProcThreadAttributeList(list,2,0,&size)); }
    void set(DWORD_PTR key, void* data, SIZE_T size) { require(UpdateProcThreadAttribute(list,0,key,data,size,nullptr,nullptr)); }
    ~Attributes() { if(list) DeleteProcThreadAttributeList(list); }
};
void cancelled(HANDLE event) { if(WaitForSingleObject(event,0)==WAIT_OBJECT_0) throw Failure{ERROR_CANCELLED}; }
void copy_input(const wchar_t* source,const std::wstring& destination,HANDLE cancel) {
    // EN-US: Cancelling a caller may unlink its temporary original while this reader finishes closing.
    // PT-BR: O cancelamento pode remover o original temporário enquanto este leitor termina de fechar.
    // EN-US: Allow delete sharing only here; the separate staged input remains sealed against writes/deletion.
    // PT-BR: Permite compartilhar exclusão só aqui; a cópia isolada continua selada contra escrita/exclusão.
    Handle input(CreateFileW(source,GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_DELETE,nullptr,OPEN_EXISTING,FILE_FLAG_SEQUENTIAL_SCAN,nullptr)); require(input.value!=INVALID_HANDLE_VALUE);
    LARGE_INTEGER length{}; require(GetFileSizeEx(input.value,&length)); require(length.QuadPart>=0&&length.QuadPart<=256LL*1024*1024);
    Handle output(CreateFileW(destination.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr)); require(output.value!=INVALID_HANDLE_VALUE);
    char bytes[65536]; DWORD read=0,written=0; ULONGLONG total=0;
    for(;;) { cancelled(cancel); require(ReadFile(input.value,bytes,sizeof(bytes),&read,nullptr)); if(!read) break;
        total+=read; require(total<=256ULL*1024*1024); require(WriteFile(output.value,bytes,read,&written,nullptr)&&written==read); }
}
void pipe(Handle& read,Handle& write) {
    SECURITY_ATTRIBUTES security{sizeof(security),nullptr,TRUE}; HANDLE r=nullptr,w=nullptr;
    require(CreatePipe(&r,&w,&security,65536)); read.reset(r); write.reset(w);
    require(SetHandleInformation(read.value,HANDLE_FLAG_INHERIT,0));
}
std::vector<unsigned char> token_info(HANDLE token,TOKEN_INFORMATION_CLASS cls) {
    DWORD size=0; GetTokenInformation(token,cls,nullptr,0,&size); require(size>0&&size<65536);
    std::vector<unsigned char> bytes(size); require(GetTokenInformation(token,cls,bytes.data(),size,&size)); return bytes;
}
void verify_token(HANDLE process,PSID profile,PSID capability) {
    HANDLE raw=nullptr; require(OpenProcessToken(process,TOKEN_QUERY,&raw)); Handle token(raw);
    auto isolated=token_info(token.value,TokenIsAppContainer); require(*reinterpret_cast<DWORD*>(isolated.data())==1);
    auto app=token_info(token.value,TokenAppContainerSid);
    require(EqualSid(reinterpret_cast<TOKEN_APPCONTAINER_INFORMATION*>(app.data())->TokenAppContainer,profile));
    auto caps=token_info(token.value,TokenCapabilities); auto groups=reinterpret_cast<TOKEN_GROUPS*>(caps.data());
    require(groups->GroupCount==1&&EqualSid(groups->Groups[0].Sid,capability));
    auto integrity=token_info(token.value,TokenIntegrityLevel); PSID label=reinterpret_cast<TOKEN_MANDATORY_LABEL*>(integrity.data())->Label.Sid;
    require(*GetSidSubAuthority(label,*GetSidSubAuthorityCount(label)-1)<=SECURITY_MANDATORY_LOW_RID);
}
struct Job {
    Handle handle; Job():handle(CreateJobObjectW(nullptr,nullptr)) {
        require(handle.value!=nullptr); JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits{};
        limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE|JOB_OBJECT_LIMIT_ACTIVE_PROCESS|JOB_OBJECT_LIMIT_PROCESS_MEMORY|JOB_OBJECT_LIMIT_JOB_MEMORY|JOB_OBJECT_LIMIT_JOB_TIME|JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        limits.BasicLimitInformation.ActiveProcessLimit=4;
        limits.BasicLimitInformation.PerJobUserTimeLimit.QuadPart=15LL*10000000;
        limits.ProcessMemoryLimit=512ULL<<20; limits.JobMemoryLimit=768ULL<<20;
        require(SetInformationJobObject(handle.value,JobObjectExtendedLimitInformation,&limits,sizeof(limits)));
    }
    void stop() {
        if(!handle.value) return;
        TerminateJobObject(handle.value,ERROR_CANCELLED);
        for(int i=0;i<200;++i) { JOBOBJECT_BASIC_ACCOUNTING_INFORMATION info{};
            if(QueryInformationJobObject(handle.value,JobObjectBasicAccountingInformation,&info,sizeof(info),nullptr)&&info.ActiveProcesses==0) break;
            Sleep(5); }
    }
    ~Job() { stop(); }
};
using Sink=void (*)(void*,int,const unsigned char*,size_t);
bool drain(HANDLE pipe,int channel,size_t& total,size_t limit,Sink sink,void* context) {
    DWORD available=0; if(!PeekNamedPipe(pipe,nullptr,0,nullptr,&available,nullptr)) { require(GetLastError()==ERROR_BROKEN_PIPE); return false; }
    char bytes[65536]; DWORD read=0;
    if(available) { require(ReadFile(pipe,bytes,std::min<DWORD>(available,sizeof(bytes)),&read,nullptr));
        if(read>limit-total) throw Failure{ERROR_BUFFER_OVERFLOW};
        total+=read;
        sink(context,channel,reinterpret_cast<unsigned char*>(bytes),read); return read!=0; }
    return false;
}
}

// EN-US: This C ABI stays narrow; Rust owns cancellation and bounded result buffers.
// PT-BR: Esta ABI C é restrita; o Rust controla cancelamento e buffers limitados de resultado.
extern "C" DWORD papo_media_prepare_runtime(const wchar_t* runtime) noexcept {
    try { Capability capability; runtime_access(runtime,capability.sid); return 0; }
    catch(const Failure& error) { return error.code; }
    catch(...) { return ERROR_GEN_FAILURE; }
}
extern "C" DWORD papo_media_run(const wchar_t* runtime,const wchar_t* source,const wchar_t* name,
    const wchar_t* script,HANDLE cancel,DWORD timeout,Sink sink,void* context,DWORD* exit_code) noexcept {
    try {
        cancelled(cancel); const ULONGLONG started=GetTickCount64();
        Capability capability; runtime_access(runtime,capability.sid); cancelled(cancel); cleanup_stale();
        Profile profile(name); profile.locate();
        Input staging(profile.name); Job job; Handle process,thread,stdout_read,stdout_write,stderr_read,stderr_write,stdin_read,stdin_write;
        // EN-US: Input is a separate read-only grant, never the source directory or profile scratch.
        // PT-BR: A entrada recebe permissão separada de só leitura, nunca a pasta original ou o scratch do perfil.
        const std::wstring input=staging.folder;
        // Remove inherited package-write rights from this subtree; keep the desktop user's full access.
        // Remove a escrita herdada do pacote nesta subárvore; mantém acesso total do usuário desktop.
        HANDLE raw=nullptr; require(OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&raw)); Handle token(raw);
        auto user=token_info(token.value,TokenUser); PSID user_sid=reinterpret_cast<TOKEN_USER*>(user.data())->User.Sid;
        Local system; require(ConvertStringSidToSidW(L"S-1-5-18",&system.value));
        EXPLICIT_ACCESSW entries[4]{};
        for(auto& e:entries) { e.grfAccessMode=SET_ACCESS; e.grfInheritance=SUB_CONTAINERS_AND_OBJECTS_INHERIT; e.Trustee.TrusteeForm=TRUSTEE_IS_SID; }
        entries[0].Trustee.ptstrName=static_cast<LPWSTR>(user_sid); entries[0].grfAccessPermissions=FILE_ALL_ACCESS;
        entries[1].Trustee.ptstrName=static_cast<LPWSTR>(profile.sid); entries[1].grfAccessPermissions=FILE_GENERIC_READ|FILE_GENERIC_EXECUTE;
        entries[2].Trustee.ptstrName=static_cast<LPWSTR>(profile.sid); entries[2].grfAccessMode=DENY_ACCESS; entries[2].grfAccessPermissions=write_rights;
        entries[3].Trustee.ptstrName=static_cast<LPWSTR>(system.value); entries[3].grfAccessPermissions=FILE_ALL_ACCESS;
        PACL acl=nullptr; status(SetEntriesInAclW(4,entries,nullptr,&acl)); Local acl_owner; acl_owner.value=acl;
        status(SetNamedSecurityInfoW(const_cast<LPWSTR>(input.c_str()),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION|PROTECTED_DACL_SECURITY_INFORMATION,nullptr,nullptr,acl,nullptr));
        staging.lock();
        const auto media=staging.file(L"media"); copy_input(source,media,cancel); staging.seal(media);
        const auto scratch=profile.folder+L"\\AC\\Temp"; CreateDirectoryW((profile.folder+L"\\AC").c_str(),nullptr); CreateDirectoryW(scratch.c_str(),nullptr);
        grant(scratch,profile.sid,FILE_ALL_ACCESS,SUB_CONTAINERS_AND_OBJECTS_INHERIT);
        // EN-US: Private codec copies stay in profile scratch; the sealed media and original runtime remain read-only.
        // PT-BR: Cópias privadas dos codecs ficam no scratch do perfil; a mídia selada e o runtime original continuam só leitura.
        const auto plugins=profile.folder+L"\\papo-codecs"; require(CreateDirectoryW(plugins.c_str(),nullptr));
        status(SetNamedSecurityInfoW(const_cast<LPWSTR>(plugins.c_str()),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION|PROTECTED_DACL_SECURITY_INFORMATION,nullptr,nullptr,acl,nullptr));
        for(const wchar_t* plugin:{L"coreelements",L"typefindfunctions",L"playback",L"app",L"videoconvertscale",L"videorate",L"isomp4",L"matroska",L"libav",L"vpx",L"videoparsersbad",L"audioparsers",L"audioconvert",L"opus",L"vorbis"}) {
            const auto filename=L"libgst"+std::wstring(plugin)+L".dll";
            copy_input((std::wstring(runtime)+L"\\lib\\gstreamer-1.0\\"+filename).c_str(),plugins+L"\\"+filename,cancel);
        }
        pipe(stdout_read,stdout_write); pipe(stderr_read,stderr_write); pipe(stdin_read,stdin_write); stdin_write.reset();
        require(SetHandleInformation(stdin_read.value,HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT));
        HANDLE inherited[]={stdin_read.value,stdout_write.value,stderr_write.value};
        SID_AND_ATTRIBUTES cap{capability.sid,SE_GROUP_ENABLED}; SECURITY_CAPABILITIES security{profile.sid,&cap,1,0};
        Attributes attributes; attributes.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,&security,sizeof(security));
        attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST,inherited,sizeof(inherited));
        STARTUPINFOEXW startup{}; startup.StartupInfo.cb=sizeof(startup); startup.lpAttributeList=attributes.list;
        startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES; startup.StartupInfo.hStdInput=inherited[0]; startup.StartupInfo.hStdOutput=inherited[1]; startup.StartupInfo.hStdError=inherited[2];
        const std::wstring root(runtime),python=root+L"\\bin\\python.exe";
        auto line=quote(python)+L" -s -P -B -u -c "+quote(script)+L" "+quote(media);
        require(line.size()<30000);
        wchar_t windows[MAX_PATH]; require(GetWindowsDirectoryW(windows,MAX_PATH)>0);
        std::vector<std::wstring> variables={L"GI_TYPELIB_PATH="+root+L"\\lib\\girepository-1.0",L"GST_DEBUG=0",L"GST_PLUGIN_SYSTEM_PATH_1_0="+plugins,L"GST_REGISTRY_1_0="+scratch+L"\\registry.bin",L"GST_REGISTRY_FORK=no",L"HOME="+scratch,L"LOCALAPPDATA="+scratch,L"PATH="+root+L"\\bin;"+windows+L"\\System32",L"PYTHONHOME="+root,L"SYSTEMROOT="+std::wstring(windows),L"TEMP="+scratch,L"TMP="+scratch,L"USERPROFILE="+scratch};
        std::vector<wchar_t> environment; for(const auto& value:variables) { environment.insert(environment.end(),value.begin(),value.end()); environment.push_back(0); } environment.push_back(0);
        PROCESS_INFORMATION child{}; cancelled(cancel);
        require(CreateProcessW(python.c_str(),line.data(),nullptr,nullptr,TRUE,CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|EXTENDED_STARTUPINFO_PRESENT,environment.data(),scratch.c_str(),&startup.StartupInfo,&child));
        process.reset(child.hProcess); thread.reset(child.hThread);
        // EN-US: Never resume until token validation and job assignment both succeed.
        // PT-BR: Só retoma após validar o token e associar o processo ao job.
        try { require(AssignProcessToJobObject(job.handle.value,process.value)); verify_token(process.value,profile.sid,capability.sid);
            require(ResumeThread(thread.value)!=static_cast<DWORD>(-1)); }
        catch(...) { TerminateProcess(process.value,ERROR_ACCESS_DENIED); throw; }
        stdout_write.reset(); stderr_write.reset(); stdin_read.reset(); thread.reset();
        size_t out=0,err=0;
        for(;;) {
            cancelled(cancel); if(GetTickCount64()-started>timeout) throw Failure{WAIT_TIMEOUT};
            // EN-US: Drain available output immediately; only idle pipes need a timed wait.
            // PT-BR: Drena a saída disponível imediatamente; só pipes ociosos precisam de espera.
            const bool data=drain(stdout_read.value,1,out,(48ULL<<20)+4096,sink,context);
            const bool errors=drain(stderr_read.value,2,err,65536,sink,context);
            if(WaitForSingleObject(process.value,data||errors?0:10)==WAIT_OBJECT_0) break;
        }
        require(GetExitCodeProcess(process.value,exit_code)); job.stop();
        // EN-US: After tree termination there can be at most the bounded pipe-buffer remainder.
        // PT-BR: Após encerrar a árvore resta apenas a quantidade limitada dos buffers de pipe.
        for(int i=0;i<4;++i) { drain(stdout_read.value,1,out,(48ULL<<20)+4096,sink,context); drain(stderr_read.value,2,err,65536,sink,context); }
        process.reset(); stdout_read.reset(); stderr_read.reset(); job.handle.reset();
        if(!staging.cleanup()||!profile.cleanup()) throw Failure{ERROR_DIR_NOT_EMPTY};
        return 0;
    } catch(const Failure& error) { return error.code; }
      catch(...) { return ERROR_GEN_FAILURE; }
}

// EN-US: Acceptance/cleanup addresses only a validated Papo-owned disposable profile name.
// PT-BR: Aceitação/limpeza trata apenas nomes validados de perfis descartáveis pertencentes ao Papo.
static bool own_profile(const wchar_t* name) {
    const std::wstring value(name);
    if(value.rfind(L"Papo.Media.",0)!=0 || value.size()>=64) return false;
    const auto separator=value.find(L'.',11);
    if(separator==std::wstring::npos || separator==11 || value.size()-separator-1!=36) return false;
    for(size_t i=11;i<separator;++i) if(value[i]<L'0'||value[i]>L'9') return false;
    for(size_t i=separator+1;i<value.size();++i) if(!((value[i]>=L'0'&&value[i]<=L'9')||(value[i]>=L'a'&&value[i]<=L'f')||value[i]==L'-')) return false;
    return true;
}
extern "C" DWORD papo_media_profile_exists(const wchar_t* name) noexcept {
    if(!own_profile(name)) return 1;
    PSID sid=nullptr; if(FAILED(DeriveAppContainerSidFromAppContainerName(name,&sid))) return 1;
    LPWSTR text=nullptr; if(!ConvertSidToStringSidW(sid,&text)) { FreeSid(sid); return 1; }
    PWSTR folder=nullptr; const HRESULT result=GetAppContainerFolderPath(text,&folder);
    const bool exists=SUCCEEDED(result)&&GetFileAttributesW(folder)!=INVALID_FILE_ATTRIBUTES;
    if(folder) CoTaskMemFree(folder);
    LocalFree(text); FreeSid(sid); if(exists) return 1;
    try { return GetFileAttributesW(input_folder(name).c_str())!=INVALID_FILE_ATTRIBUTES?1:0; } catch(...) { return 1; }
}
extern "C" DWORD papo_media_profile_remove(const wchar_t* name) noexcept {
    if(!own_profile(name)) return ERROR_ACCESS_DENIED;
    return static_cast<DWORD>(DeleteAppContainerProfile(name));
}

static bool dead_owner(const wchar_t* name) {
    if(!own_profile(name)) return false;
    wchar_t* end=nullptr; const unsigned long pid=wcstoul(name+11,&end,10);
    if(!pid||pid==GetCurrentProcessId()||!end||*end!=L'.') return false;
    HANDLE process=OpenProcess(SYNCHRONIZE,FALSE,pid);
    if(process) { const bool dead=WaitForSingleObject(process,0)==WAIT_OBJECT_0; CloseHandle(process); return dead; }
    return GetLastError()==ERROR_INVALID_PARAMETER;
}
static void remove_stale(const wchar_t* name) {
    if(!dead_owner(name)) return;
    try {
        const auto input=input_folder(name); const auto attributes=GetFileAttributesW(input.c_str());
        if(attributes!=INVALID_FILE_ATTRIBUTES) {
            // EN-US: Never follow reparse points during host-side orphan cleanup.
            // PT-BR: Nunca segue pontos de nova análise ao limpar órfãos pelo processo host.
            if(attributes&FILE_ATTRIBUTE_REPARSE_POINT) { RemoveDirectoryW(input.c_str()); }
            else {
                DeleteFileW((input+L"\\media").c_str()); const auto plugins=input+L"\\plugins";
                const auto plugin_attributes=GetFileAttributesW(plugins.c_str());
                if(plugin_attributes!=INVALID_FILE_ATTRIBUTES&&!(plugin_attributes&FILE_ATTRIBUTE_REPARSE_POINT)) {
                    WIN32_FIND_DATAW plugin{}; HANDLE files=FindFirstFileW((plugins+L"\\libgst*.dll").c_str(),&plugin);
                    if(files!=INVALID_HANDLE_VALUE) { do { if(!(plugin.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY)) DeleteFileW((plugins+L"\\"+plugin.cFileName).c_str()); } while(FindNextFileW(files,&plugin)); FindClose(files); }
                }
                RemoveDirectoryW(plugins.c_str()); RemoveDirectoryW(input.c_str());
            }
        }
        DeleteAppContainerProfile(name);
    } catch(...) {}
}
static void cleanup_stale() {
    // EN-US: Inspect only our strict disposable prefixes, including a sealed input whose profile was already removed.
    // PT-BR: Inspeciona só nossos prefixos descartáveis restritos, incluindo entradas cujo perfil já foi removido.
    DWORD size=GetEnvironmentVariableW(L"LOCALAPPDATA",nullptr,0);
    if(size&&size<=32768) {
        std::vector<wchar_t> folder(size);
        if(GetEnvironmentVariableW(L"LOCALAPPDATA",folder.data(),size)) {
            WIN32_FIND_DATAW entry{}; HANDLE search=FindFirstFileW((std::wstring(folder.data())+L"\\Packages\\Papo.Media.*").c_str(),&entry);
            if(search!=INVALID_HANDLE_VALUE) { do { if(!(entry.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT)) remove_stale(entry.cFileName); } while(FindNextFileW(search,&entry)); FindClose(search); }
        }
    }
    wchar_t temp[32768]; const DWORD length=GetTempPathW(32768,temp); if(!length||length>=32768) return;
    WIN32_FIND_DATAW entry{}; HANDLE search=FindFirstFileW((std::wstring(temp)+L"papo-input-Papo.Media.*").c_str(),&entry);
    if(search!=INVALID_HANDLE_VALUE) { do { remove_stale(entry.cFileName+11); } while(FindNextFileW(search,&entry)); FindClose(search); }
}
