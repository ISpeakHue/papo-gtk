/* EN-US: ABI prefixes used by the protocol-only toast adapter. Interface IDs/slot order
 * follow Windows.UI.Notifications and Windows.Data.Xml.Dom metadata. This avoids
 * MinGW's unrelated IReference<boolean>/BYTE template collision. Never extend a
 * prefix without preserving every preceding vtable slot.
 * PT-BR: Prefixos ABI usados pelo adaptador de avisos de protocolo. IDs/ordem seguem os
 * metadados Windows.UI.Notifications e Windows.Data.Xml.Dom. Evita a colisão
 * de templates IReference<boolean>/BYTE do MinGW. Preserve todos os slots anteriores. */
#pragma once
#include <inspectable.h>
struct XmlIO : IInspectable {virtual HRESULT STDMETHODCALLTYPE LoadXml(HSTRING)=0;};
struct ToastFactory : IInspectable {virtual HRESULT STDMETHODCALLTYPE CreateToastNotification(IInspectable*,IInspectable**)=0;};
struct ToastNames : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE put_Tag(HSTRING)=0;
    virtual HRESULT STDMETHODCALLTYPE get_Tag(HSTRING*)=0;
    virtual HRESULT STDMETHODCALLTYPE put_Group(HSTRING)=0;
};
struct ToastNotifier : IInspectable {virtual HRESULT STDMETHODCALLTYPE Show(IInspectable*)=0;};
struct ToastManager : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE CreateToastNotifier(ToastNotifier**)=0;
    virtual HRESULT STDMETHODCALLTYPE CreateToastNotifierWithId(HSTRING,ToastNotifier**)=0;
};
struct ToastHistory : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE RemoveGroup(HSTRING)=0;
    virtual HRESULT STDMETHODCALLTYPE RemoveGroupWithId(HSTRING,HSTRING)=0;
    virtual HRESULT STDMETHODCALLTYPE RemoveGroupedTagWithId(HSTRING,HSTRING,HSTRING)=0;
};
struct ToastManager2 : IInspectable {virtual HRESULT STDMETHODCALLTYPE get_History(ToastHistory**)=0;};
struct ToastVector : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE GetAt(UINT32,IInspectable**)=0;
    virtual HRESULT STDMETHODCALLTYPE get_Size(UINT32*)=0;
};
struct ToastHistory2 : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE GetHistory(ToastVector**)=0;
    virtual HRESULT STDMETHODCALLTYPE GetHistoryWithId(HSTRING,ToastVector**)=0;
};
