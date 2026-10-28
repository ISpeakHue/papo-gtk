// EN-US: Synthetic ABI/ownership/lifetime checks. Never opens a picker or reads the desktop.
// PT-BR: Verifica ABI/propriedade/ciclo de vida sintéticos. Não abre seletor nem lê a tela.
#include "../src/platform/capture/windows.cpp"
#include <cassert>
#include <cstdio>

int main() {
    assert(papo_capture_abi() == 1);
    assert(papo_capture_state(nullptr) == FAILED);
    assert(papo_capture_close(nullptr) == 1);
    assert(valid_size({3840, 2160}));
    assert(!valid_size({0, 360}) && !valid_size({8192, 8192}) && !valid_size({INT32_MAX, 1}));
    HWND owner = CreateWindowExW(0, L"STATIC", L"Papo synthetic owner", WS_OVERLAPPED,
        0, 0, 100, 100, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    assert(owner && valid_owner(owner) && !valid_owner(nullptr));
    DestroyWindow(owner); assert(!valid_owner(owner));
    Capture sample;
    sample.latest = {1, 2, 3, 4}; sample.width = 1; sample.height = 1;
    BYTE buffer[4]{}; UINT32 width{}, height{};
    // EN-US: Pending/cancelled/closed sources may not expose even a queued frame.
    // PT-BR: Fontes pendentes/canceladas/fechadas não expõem nem um quadro enfileirado.
    for (int state : {PICKING, CANCELLED, ENDED, FAILED}) {
        sample.state = state;
        assert(papo_capture_read(&sample, buffer, 4, &width, &height) == 0);
    }
    sample.state = RUNNING;
    assert(papo_capture_read(&sample, nullptr, 0, &width, &height) == -2);
    assert(width == 1 && height == 1);
    assert(papo_capture_read(&sample, buffer, 3, &width, &height) == -2);
    assert(papo_capture_read(&sample, buffer, 4, &width, &height) == 4);
    assert(buffer[0] == 1 && buffer[3] == 4);
    assert(papo_capture_read(&sample, buffer, 4, &width, &height) == 0);
    sample.latest = {5, 6, 7, 8}; sample.closed->store(true);
    assert(papo_capture_read(&sample, buffer, 4, &width, &height) == 0);
    assert(papo_capture_state(&sample) == ENDED);
    auto capture = new Capture();
    capture->thread = CreateThread(nullptr, 0, [](void* ptr)->DWORD {
        auto source = static_cast<Capture*>(ptr); WaitForSingleObject(source->stop, INFINITE); return 0;
    }, capture, 0, nullptr);
    assert(capture->thread && papo_capture_close(capture) == 1);
    HANDLE release = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    auto slow = new Capture();
    slow->thread = CreateThread(nullptr, 0, [](void* event)->DWORD {
        WaitForSingleObject(static_cast<HANDLE>(event), INFINITE); return 0;
    }, release, 0, nullptr);
    assert(release && slow->thread);
    auto started = GetTickCount64();
    assert(papo_capture_close(slow) == 0);
    auto elapsed = GetTickCount64() - started;
    assert(elapsed >= 1500 && elapsed < 8000);
    SetEvent(release);
    assert(papo_capture_close(slow) == 1);
    CloseHandle(release);
    puts("PASS: owned HWND, bounded frame copy, no pre-consent/closed frames, joined teardown; no capture performed");
    return 0;
}
