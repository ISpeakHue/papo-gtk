// EN-US: WinRT ABI prefixes from Microsoft Windows metadata; preserve every preceding slot.
// PT-BR: Prefixos ABI dos metadados Windows da Microsoft; preserve todos os slots anteriores.
// https://github.com/microsoft/windows-rs/tree/master/crates/libs/windows/src/Windows/Graphics/Capture
#pragma once
#include <inspectable.h>

struct CaptureSize { INT32 width, height; };
struct CaptureClosable : IInspectable { virtual HRESULT STDMETHODCALLTYPE Close() = 0; };
struct CaptureItem;
struct CaptureOperation : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE put_Completed(IUnknown*) = 0;
    virtual HRESULT STDMETHODCALLTYPE get_Completed(IUnknown**) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetResults(CaptureItem**) = 0;
};
struct CapturePicker : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE PickSingleItemAsync(CaptureOperation**) = 0;
};
struct CaptureClosed : IUnknown {
    virtual HRESULT STDMETHODCALLTYPE Invoke(IInspectable*, IInspectable*) = 0;
};
struct CaptureItem : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE get_DisplayName(HSTRING*) = 0;
    virtual HRESULT STDMETHODCALLTYPE get_Size(CaptureSize*) = 0;
    virtual HRESULT STDMETHODCALLTYPE add_Closed(CaptureClosed*, INT64*) = 0;
    virtual HRESULT STDMETHODCALLTYPE remove_Closed(INT64) = 0;
};
struct CaptureSession : IInspectable { virtual HRESULT STDMETHODCALLTYPE StartCapture() = 0; };
struct CaptureSupported : IInspectable { virtual HRESULT STDMETHODCALLTYPE IsSupported(boolean*) = 0; };
struct CaptureFrame : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE get_Surface(IInspectable**) = 0;
    virtual HRESULT STDMETHODCALLTYPE get_SystemRelativeTime(INT64*) = 0;
    virtual HRESULT STDMETHODCALLTYPE get_ContentSize(CaptureSize*) = 0;
};
struct CapturePool : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE Recreate(IInspectable*, INT32, INT32, CaptureSize) = 0;
    virtual HRESULT STDMETHODCALLTYPE TryGetNextFrame(CaptureFrame**) = 0;
    virtual HRESULT STDMETHODCALLTYPE add_FrameArrived(IUnknown*, INT64*) = 0;
    virtual HRESULT STDMETHODCALLTYPE remove_FrameArrived(INT64) = 0;
    virtual HRESULT STDMETHODCALLTYPE CreateCaptureSession(CaptureItem*, CaptureSession**) = 0;
};
struct CapturePoolFactory : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE CreateFreeThreaded(IInspectable*, INT32, INT32, CaptureSize, CapturePool**) = 0;
};
struct CaptureDxgiAccess : IUnknown {
    virtual HRESULT STDMETHODCALLTYPE GetInterface(REFIID, void**) = 0;
};
