// EN-US: WGC objects never leave this worker DLL. Only copied BGRA frames cross its C ABI.
// PT-BR: Objetos WGC não saem desta DLL do worker. Só quadros BGRA copiados cruzam a ABI C.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define UNICODE
#define _UNICODE
#include <windows.h>
#include <roapi.h>
#include <winstring.h>
#include <asyncinfo.h>
#include <shobjidl.h>
#include <d3d11.h>
#include <dxgi.h>
#include <wrl/client.h>
#include <atomic>
#include <memory>
#include <mutex>
#include <vector>
#include "windows_abi.h"

using Microsoft::WRL::ComPtr;
namespace {
constexpr size_t MAX_BYTES = 64 * 1024 * 1024;
constexpr INT32 BGRA = 87;
enum State { PICKING = 0, RUNNING = 1, CANCELLED = 2, ENDED = 3, FAILED = 4 };
void check(HRESULT hr) { if (FAILED(hr)) throw hr; }
GUID guid(const wchar_t* text) { GUID id{}; check(CLSIDFromString(text, &id)); return id; }
template<class T> ComPtr<T> query(IUnknown* object, const wchar_t* id) {
    ComPtr<T> result; check(object->QueryInterface(guid(id), reinterpret_cast<void**>(result.GetAddressOf()))); return result;
}
template<class T> ComPtr<T> factory(const wchar_t* name, const wchar_t* id) {
    HSTRING text{}; check(WindowsCreateString(name, static_cast<UINT32>(wcslen(name)), &text));
    ComPtr<T> result;
    HRESULT hr = RoGetActivationFactory(text, guid(id), reinterpret_cast<void**>(result.GetAddressOf()));
    WindowsDeleteString(text); check(hr); return result;
}
void close_object(IUnknown* object) noexcept {
    if (!object) return;
    try { query<CaptureClosable>(object, L"{30D5A829-7FA4-4026-83BB-D75BAE4EA99E}")->Close(); } catch (...) {}
}
struct FrameGuard {
    ComPtr<CaptureFrame> frame;
    ~FrameGuard() { close_object(frame.Get()); }
};
struct AsyncGuard {
    ComPtr<IAsyncInfo> info;
    ~AsyncGuard() {
        if (!info) return;
        AsyncStatus status{};
        if (SUCCEEDED(info->get_Status(&status)) && status == Started) info->Cancel();
        info->Close();
    }
};
struct ClosedHandler final : CaptureClosed {
    std::atomic<ULONG> refs{1};
    std::shared_ptr<std::atomic<bool>> closed;
    explicit ClosedHandler(std::shared_ptr<std::atomic<bool>> flag) : closed(std::move(flag)) {}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        // EN-US: TypedEventHandler<GraphicsCaptureItem, IInspectable>, plus agile callback identity.
        // PT-BR: TypedEventHandler<GraphicsCaptureItem, IInspectable> e identidade de callback ágil.
        if (id == IID_IUnknown || id == guid(L"{94EA2B94-E9CC-49E0-C0FF-EE64CA8F5B90}") ||
            id == guid(L"{E9C610C0-A68C-5BD9-8021-8589346EEEE2}")) {
            *out = static_cast<CaptureClosed*>(this); AddRef(); return S_OK;
        }
        return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++refs; }
    ULONG STDMETHODCALLTYPE Release() override { auto left = --refs; if (!left) delete this; return left; }
    HRESULT STDMETHODCALLTYPE Invoke(IInspectable*, IInspectable*) override { closed->store(true); return S_OK; }
};
struct Capture {
    HANDLE stop = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    HANDLE thread = nullptr;
    std::atomic<int> state{PICKING};
    std::shared_ptr<std::atomic<bool>> closed = std::make_shared<std::atomic<bool>>(false);
    std::mutex mutex;
    std::vector<BYTE> latest;
    UINT32 width = 0, height = 0;
    ~Capture() { if (thread) CloseHandle(thread); if (stop) CloseHandle(stop); }
    bool stopped() const { return WaitForSingleObject(stop, 0) == WAIT_OBJECT_0; }
};
bool valid_size(CaptureSize size) {
    return size.width > 0 && size.height > 0 && size.width <= 8192 && size.height <= 8192 &&
        static_cast<UINT64>(size.width) * size.height * 4 <= MAX_BYTES;
}
bool supported() {
    auto api = factory<CaptureSupported>(L"Windows.Graphics.Capture.GraphicsCaptureSession",
        L"{2224A540-5974-49AA-B232-0882536F4CB5}");
    boolean value = false; check(api->IsSupported(&value));
    if (!value) return false;
    factory<CapturePoolFactory>(L"Windows.Graphics.Capture.Direct3D11CaptureFramePool",
        L"{589B103F-6BBC-5DF5-A991-02E28B3B66D5}");
    return true;
}
LRESULT CALLBACK owner_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    auto capture = reinterpret_cast<Capture*>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
    if (msg == WM_CLOSE && capture) { SetEvent(capture->stop); return 0; }
    return DefWindowProcW(hwnd, msg, wp, lp);
}
void pump() {
    MSG msg{};
    while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) { TranslateMessage(&msg); DispatchMessageW(&msg); }
}
bool valid_owner(HWND owner) {
    DWORD pid = 0;
    return IsWindow(owner) && GetWindowThreadProcessId(owner, &pid) == GetCurrentThreadId() && pid == GetCurrentProcessId();
}
ComPtr<CaptureItem> pick(Capture& capture, HWND owner) {
    if (capture.stopped()) return {};
    HSTRING name{};
    check(WindowsCreateString(L"Windows.Graphics.Capture.GraphicsCapturePicker", 46, &name));
    ComPtr<IInspectable> object;
    HRESULT hr = RoActivateInstance(name, &object); WindowsDeleteString(name); check(hr);
    auto picker = query<CapturePicker>(object.Get(), L"{5A1711B3-AD79-4B4A-9336-1318FDDE3539}");
    auto init = query<IInitializeWithWindow>(object.Get(), L"{3E68D4BD-7135-4D10-8018-9FB6D9F33FA1}");
    if (!valid_owner(owner)) throw E_INVALIDARG;
    check(init->Initialize(owner));
    ComPtr<CaptureOperation> operation; check(picker->PickSingleItemAsync(&operation));
    AsyncGuard pending{query<IAsyncInfo>(operation.Get(), L"{00000036-0000-0000-C000-000000000046}")};
    // EN-US: Leave time for first-frame setup and renegotiation before the shared UI deadline.
    // PT-BR: Reserva tempo para o primeiro quadro e renegociação antes do limite da interface.
    auto deadline = GetTickCount64() + 90000;
    AsyncStatus status = Started;
    while (!capture.stopped() && GetTickCount64() < deadline) {
        pump(); check(pending.info->get_Status(&status));
        if (status != Started) break;
        MsgWaitForMultipleObjects(1, &capture.stop, FALSE, 20, QS_ALLINPUT);
    }
    if (capture.stopped() || status == Started || status == Canceled) return {};
    ComPtr<CaptureItem> item;
    hr = operation->GetResults(&item); check(hr);
    return item;
}
void capture_frames(Capture& capture, CaptureItem* item) {
    CaptureSize size{}; check(item->get_Size(&size));
    if (!valid_size(size)) throw E_INVALIDARG;
    ComPtr<ID3D11Device> device; ComPtr<ID3D11DeviceContext> context;
    check(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, &context));
    auto dxgi = query<IDXGIDevice>(device.Get(), L"{54EC77FA-1377-44E6-8C32-88FD5F44C84C}");
    // EN-US: Resolve only the Windows system D3D11 DLL, never the working directory or PATH.
    // PT-BR: Resolve só a DLL D3D11 do sistema Windows, nunca o diretório atual ou PATH.
    HMODULE d3d = LoadLibraryExW(L"d3d11.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!d3d) throw E_NOINTERFACE;
    using WrapDevice = HRESULT (WINAPI*)(IDXGIDevice*, IInspectable**);
    auto wrap = reinterpret_cast<WrapDevice>(reinterpret_cast<void*>(GetProcAddress(d3d, "CreateDirect3D11DeviceFromDXGIDevice")));
    ComPtr<IInspectable> winrt_device;
    HRESULT wrapped = wrap ? wrap(dxgi.Get(), &winrt_device) : E_NOINTERFACE;
    FreeLibrary(d3d); check(wrapped);
    auto capture_device = query<IInspectable>(winrt_device.Get(), L"{A37624AB-8D5F-4650-9D3E-9EAE3D9BC670}");
    auto api = factory<CapturePoolFactory>(L"Windows.Graphics.Capture.Direct3D11CaptureFramePool",
        L"{589B103F-6BBC-5DF5-A991-02E28B3B66D5}");
    ComPtr<CapturePool> pool;
    check(api->CreateFreeThreaded(capture_device.Get(), BGRA, 2, size, &pool));
    ComPtr<CaptureSession> session;
    ComPtr<CaptureClosed> handler; handler.Attach(new ClosedHandler(capture.closed));
    INT64 token{}; bool registered = false;
    try {
        check(item->add_Closed(handler.Get(), &token)); registered = true;
        check(pool->CreateCaptureSession(item, &session));
        if (!capture.stopped() && !capture.closed->load()) {
            // EN-US: Only the item returned by the picker can start capture. Keep the system border.
            // PT-BR: Só o item retornado pelo seletor inicia captura. Mantém a borda do sistema.
            check(session->StartCapture()); capture.state = RUNNING;
        }
        ComPtr<ID3D11Texture2D> staging;
        while (!capture.stopped() && !capture.closed->load()) {
            pump();
            if (WaitForSingleObject(capture.stop, 66) == WAIT_OBJECT_0) break;
            FrameGuard frame; check(pool->TryGetNextFrame(&frame.frame));
            if (!frame.frame) continue;
            CaptureSize actual{}; check(frame.frame->get_ContentSize(&actual));
            if (actual.width <= 0 || actual.height <= 0) continue;
            if (!valid_size(actual)) throw E_INVALIDARG;
            if (actual.width != size.width || actual.height != size.height) {
                // EN-US: Discard the old-size frame before recreating; never copy uninitialized margins.
                // PT-BR: Descarta o quadro de tamanho antigo antes de recriar; não copia margens não inicializadas.
                close_object(frame.frame.Get()); frame.frame.Reset(); staging.Reset();
                { std::lock_guard<std::mutex> lock(capture.mutex); capture.latest.clear(); }
                size = actual; check(pool->Recreate(capture_device.Get(), BGRA, 2, size)); continue;
            }
            ComPtr<IInspectable> surface; check(frame.frame->get_Surface(&surface));
            auto access = query<CaptureDxgiAccess>(surface.Get(), L"{A9B3D012-3DF2-4EE3-B8D1-8695F457D3C1}");
            ComPtr<ID3D11Texture2D> texture;
            check(access->GetInterface(__uuidof(ID3D11Texture2D), reinterpret_cast<void**>(texture.GetAddressOf())));
            D3D11_TEXTURE2D_DESC desc{}; texture->GetDesc(&desc);
            if (desc.Width < static_cast<UINT32>(size.width) || desc.Height < static_cast<UINT32>(size.height) ||
                desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM) throw E_INVALIDARG;
            if (!staging) {
                desc.Width = size.width; desc.Height = size.height; desc.MipLevels = 1; desc.ArraySize = 1;
                desc.SampleDesc = {1, 0}; desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ; desc.MiscFlags = 0;
                check(device->CreateTexture2D(&desc, nullptr, &staging));
            }
            D3D11_BOX box{0, 0, 0, static_cast<UINT>(size.width), static_cast<UINT>(size.height), 1};
            context->CopySubresourceRegion(staging.Get(), 0, 0, 0, 0, texture.Get(), 0, &box);
            D3D11_MAPPED_SUBRESOURCE mapped{};
            check(context->Map(staging.Get(), 0, D3D11_MAP_READ, 0, &mapped));
            if (!mapped.pData || mapped.RowPitch < static_cast<UINT32>(size.width) * 4) {
                context->Unmap(staging.Get(), 0); throw E_FAIL;
            }
            try {
                std::lock_guard<std::mutex> lock(capture.mutex);
                if (!capture.stopped() && !capture.closed->load()) {
                    capture.latest.resize(static_cast<size_t>(size.width) * size.height * 4);
                    for (INT32 row = 0; row < size.height; ++row)
                        memcpy(capture.latest.data() + static_cast<size_t>(row) * size.width * 4,
                            static_cast<BYTE*>(mapped.pData) + static_cast<size_t>(row) * mapped.RowPitch, size.width * 4);
                    capture.width = size.width; capture.height = size.height;
                }
            } catch (...) { context->Unmap(staging.Get(), 0); throw; }
            context->Unmap(staging.Get(), 0);
        }
    } catch (...) {
        close_object(session.Get()); close_object(pool.Get());
        if (registered) item->remove_Closed(token);
        throw;
    }
    close_object(session.Get()); close_object(pool.Get());
    if (registered) item->remove_Closed(token);
}
DWORD WINAPI run(void* value) {
    auto& capture = *static_cast<Capture*>(value);
    HRESULT apartment = RoInitialize(RO_INIT_SINGLETHREADED);
    HWND owner = nullptr;
    try {
        check(apartment);
        if (!supported()) throw E_NOINTERFACE;
        WNDCLASSW cls{}; cls.lpfnWndProc = owner_proc; cls.hInstance = GetModuleHandleW(nullptr);
        cls.lpszClassName = L"PapoCapturePickerOwner"; cls.hCursor = LoadCursorW(nullptr, IDC_ARROW);
        if (!RegisterClassW(&cls) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) throw E_FAIL;
        owner = CreateWindowExW(WS_EX_APPWINDOW, cls.lpszClassName, L"Papo — Compartilhar tela",
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU, CW_USEDEFAULT, CW_USEDEFAULT, 420, 100,
            nullptr, nullptr, cls.hInstance, nullptr);
        if (!owner) throw E_FAIL;
        SetWindowLongPtrW(owner, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(&capture));
        // EN-US: The temporary picker host must not itself become a capture target.
        // PT-BR: A janela temporária do seletor não pode virar uma fonte de captura.
        if (!SetWindowDisplayAffinity(owner, 0x11 /* WDA_EXCLUDEFROMCAPTURE */)) throw E_FAIL;
        ShowWindow(owner, SW_SHOWNORMAL); SetForegroundWindow(owner);
        auto item = pick(capture, owner);
        ShowWindow(owner, SW_HIDE);
        if (!item || capture.stopped()) capture.state = CANCELLED;
        else { capture_frames(capture, item.Get()); capture.state = ENDED; }
    } catch (...) { capture.state = FAILED; }
    if (owner) DestroyWindow(owner);
    { std::lock_guard<std::mutex> lock(capture.mutex); capture.latest.clear(); }
    if (SUCCEEDED(apartment)) RoUninitialize();
    return 0;
}
}
// EN-US: Versioned, process-local C ABI; no tokens, COM pointers or source names in JSON/logs.
// PT-BR: ABI C versionada e local ao processo; sem tokens, ponteiros COM ou nomes de fontes em JSON/logs.
extern "C" __declspec(dllexport) int papo_capture_abi() { return 1; }
extern "C" __declspec(dllexport) int papo_capture_supported() {
    HRESULT hr = RoInitialize(RO_INIT_MULTITHREADED);
    bool result = false;
    // EN-US: GI may have already initialized an STA; inspect support in that existing apartment.
    // PT-BR: GI pode já ter inicializado um STA; verifica suporte nesse apartment existente.
    try { if (hr != RPC_E_CHANGED_MODE) check(hr); result = supported(); } catch (...) {}
    if (SUCCEEDED(hr)) RoUninitialize();
    return result ? 1 : 0;
}
extern "C" __declspec(dllexport) void* papo_capture_start() {
    try {
        auto capture = std::make_unique<Capture>();
        if (!capture->stop) return nullptr;
        capture->thread = CreateThread(nullptr, 0, run, capture.get(), 0, nullptr);
        if (!capture->thread) return nullptr;
        return capture.release();
    } catch (...) { return nullptr; }
}
extern "C" __declspec(dllexport) int papo_capture_state(void* value) {
    if (!value) return FAILED;
    auto& capture = *static_cast<Capture*>(value);
    int state = capture.state.load();
    if (state == RUNNING && (capture.stopped() || capture.closed->load())) return ENDED;
    return state;
}
extern "C" __declspec(dllexport) int papo_capture_read(void* value, BYTE* bytes, UINT32 capacity, UINT32* width, UINT32* height) {
    if (!value || !width || !height) return -1;
    auto& capture = *static_cast<Capture*>(value);
    std::lock_guard<std::mutex> lock(capture.mutex);
    if (capture.state != RUNNING || capture.stopped() || capture.closed->load() || capture.latest.empty()) return 0;
    *width = capture.width; *height = capture.height;
    if (!bytes || capacity < capture.latest.size()) return -2;
    memcpy(bytes, capture.latest.data(), capture.latest.size());
    int length = static_cast<int>(capture.latest.size()); capture.latest.clear(); return length;
}
extern "C" __declspec(dllexport) int papo_capture_close(void* value) {
    if (!value) return 1;
    auto capture = static_cast<Capture*>(value); SetEvent(capture->stop);
    // EN-US: The caller terminates the worker if driver/WinRT teardown exceeds this bound.
    // PT-BR: O chamador encerra o worker se a liberação do driver/WinRT exceder este limite.
    if (WaitForSingleObject(capture->thread, 2000) != WAIT_OBJECT_0) return 0;
    delete capture; return 1;
}
