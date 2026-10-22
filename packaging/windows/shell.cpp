/* EN-US: Small UCRT-compatible shell/WinRT adapter; no Windows App SDK runtime.
 * PT-BR: Adaptador pequeno de shell/WinRT compatível com UCRT, sem Windows App SDK. */
#define _WIN32_WINNT 0x0A00
#define WINVER 0x0A00
#include <windows.h>
#include <shlobj.h>
#include <propkey.h>
#include <propvarutil.h>
#include <propsys.h>
#include <roapi.h>
#include <winstring.h>
#include "toast_abi.h"
#include <cstdio>
#include <string>
#include <vector>
#include <stdexcept>
#include "identity.h"

static void check(HRESULT value) { if (FAILED(value)) { char text[64];std::snprintf(text,sizeof(text),"Windows HRESULT 0x%08lx",(unsigned long)value);throw std::runtime_error(text); } }
template<class T> struct Com { T *p=nullptr;~Com(){if(p)p->Release();}T** out(){return &p;}T* operator->(){return p;} };
struct String { HSTRING value=nullptr;explicit String(const std::wstring& text){check(WindowsCreateString(text.c_str(),(UINT32)text.size(),&value));}~String(){WindowsDeleteString(value);}operator HSTRING()const{return value;} };
static GUID guid(const wchar_t* text){GUID value;check(CLSIDFromString(text,&value));return value;}
static std::wstring module_path(){std::vector<wchar_t> path(32768);DWORD size=GetModuleFileNameW(nullptr,path.data(),(DWORD)path.size());if(!size||size>=path.size())throw std::runtime_error("Cannot locate shell adapter");return std::wstring(path.data(),size);}
static std::wstring root_path(){auto path=module_path();for(int i=0;i<3;i++)path.resize(path.find_last_of(L"\\/"));return path;}
static std::wstring shortcut_path(){wchar_t path[MAX_PATH];check(SHGetFolderPathW(nullptr,CSIDL_PROGRAMS,nullptr,SHGFP_TYPE_CURRENT,path));return std::wstring(path)+L"\\Papo.lnk";}
static std::wstring registry(const std::wstring& path,const wchar_t* name){
    DWORD type=0,size=0;LSTATUS result=RegGetValueW(HKEY_CURRENT_USER,path.c_str(),name,RRF_RT_REG_SZ,&type,nullptr,&size);
    if(result==ERROR_FILE_NOT_FOUND||result==ERROR_PATH_NOT_FOUND)return L"";
    if(result!=ERROR_SUCCESS)throw std::runtime_error("Cannot read desktop registration");
    std::vector<wchar_t> value(size/sizeof(wchar_t)+1);if(RegGetValueW(HKEY_CURRENT_USER,path.c_str(),name,RRF_RT_REG_SZ,nullptr,value.data(),&size)!=ERROR_SUCCESS)throw std::runtime_error("Cannot read desktop value");return value.data();
}
static bool exists(const std::wstring& path){HKEY key;auto result=RegOpenKeyExW(HKEY_CURRENT_USER,path.c_str(),0,KEY_READ,&key);if(result==ERROR_FILE_NOT_FOUND||result==ERROR_PATH_NOT_FOUND)return false;if(result!=ERROR_SUCCESS)throw std::runtime_error("Cannot inspect registration ownership");RegCloseKey(key);return true;}
static void set(const std::wstring& path,const wchar_t* name,const std::wstring& value){HKEY key;check(HRESULT_FROM_WIN32(RegCreateKeyExW(HKEY_CURRENT_USER,path.c_str(),0,nullptr,0,KEY_SET_VALUE,nullptr,&key,nullptr)));auto result=RegSetValueExW(key,name,0,REG_SZ,(const BYTE*)value.c_str(),(DWORD)((value.size()+1)*sizeof(wchar_t)));RegCloseKey(key);check(HRESULT_FROM_WIN32(result));}
static std::vector<std::wstring> keys(){return {std::wstring(L"Software\\Classes\\")+PAPO_PROTOCOL,std::wstring(L"Software\\Classes\\AppUserModelId\\")+PAPO_APP_ID,std::wstring(L"Software\\Classes\\CLSID\\")+PAPO_TOAST_CLSID};}
static void shell_link(Com<IShellLinkW>& link,bool load){check(CoCreateInstance(CLSID_ShellLink,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(link.out())));if(load){Com<IPersistFile> file;check(link->QueryInterface(IID_PPV_ARGS(file.out())));check(file->Load(shortcut_path().c_str(),STGM_READ));}}
static std::wstring shortcut_target(){if(GetFileAttributesW(shortcut_path().c_str())==INVALID_FILE_ATTRIBUTES)return L"";Com<IShellLinkW> link;shell_link(link,true);wchar_t path[32768];check(link->GetPath(path,32768,nullptr,SLGP_RAWPATH));return path;}
static void registration(bool remove){
    const auto root=root_path(),exe=root+L"\\Papo.exe";const auto owned_keys=keys();auto shortcut=shortcut_target();
    for(const auto& key:owned_keys){if(exists(key)){if(registry(key,L"PapoOwner")!=PAPO_APP_ID)throw std::runtime_error("Existing desktop identity belongs to another application");if(remove&&registry(key,L"InstallPath")!=root)throw std::runtime_error("A newer installation owns this identity");}}
    if(!shortcut.empty()&&shortcut!=exe){if(remove||registry(owned_keys[0],L"InstallPath").empty()||shortcut!=registry(owned_keys[0],L"InstallPath")+L"\\Papo.exe")throw std::runtime_error("Existing Papo shortcut has another owner");}
    if(remove){for(const auto& key:owned_keys)if(exists(key))check(HRESULT_FROM_WIN32(RegDeleteTreeW(HKEY_CURRENT_USER,key.c_str())));if(shortcut==exe&&!DeleteFileW(shortcut_path().c_str()))throw std::runtime_error("Cannot remove shortcut");}
    else{
        for(const auto& key:owned_keys){set(key,L"PapoOwner",PAPO_APP_ID);set(key,L"InstallPath",root);}
        set(owned_keys[0],nullptr,L"URL:Papo notification");set(owned_keys[0],L"URL Protocol",L"");set(owned_keys[0]+L"\\DefaultIcon",nullptr,L"\""+exe+L"\",0");
        set(owned_keys[0]+L"\\shell\\open\\command",nullptr,L"\""+exe+L"\" --activate \"%1\"");
        set(owned_keys[1],L"DisplayName",PAPO_NAME);set(owned_keys[1],L"IconUri",root+L"\\runtime\\share\\papo-gtk\\icon.png");set(owned_keys[1],L"CustomActivator",PAPO_TOAST_CLSID);
        set(owned_keys[2],nullptr,L"Papo protocol-only toast identity");
        Com<IShellLinkW> link;shell_link(link,false);check(link->SetPath(exe.c_str()));check(link->SetWorkingDirectory(root.c_str()));check(link->SetIconLocation(exe.c_str(),0));check(link->SetDescription(L"Papo"));
        Com<IPropertyStore> properties;check(link->QueryInterface(IID_PPV_ARGS(properties.out())));
        PROPVARIANT value;PropVariantInit(&value);check(InitPropVariantFromString(PAPO_APP_ID,&value));check(properties->SetValue(PKEY_AppUserModel_ID,value));PropVariantClear(&value);
        CLSID clsid;check(CLSIDFromString(PAPO_TOAST_CLSID,&clsid));check(InitPropVariantFromCLSID(clsid,&value));check(properties->SetValue(PKEY_AppUserModel_ToastActivatorCLSID,value));PropVariantClear(&value);check(properties->Commit());
        Com<IPersistFile> file;check(link->QueryInterface(IID_PPV_ARGS(file.out())));auto temporary=shortcut_path()+L".papo-tmp.lnk";check(file->Save(temporary.c_str(),TRUE));if(!MoveFileExW(temporary.c_str(),shortcut_path().c_str(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH))throw std::runtime_error("Cannot replace Papo shortcut");
    }
    SHChangeNotify(SHCNE_ASSOCCHANGED,SHCNF_IDLIST,nullptr,nullptr);
}
static bool ticket(const wchar_t* value){if(wcslen(value)!=36)return false;for(size_t i=0;i<36;i++){if(i==8||i==13||i==18||i==23){if(value[i]!=L'-')return false;}else if(!((value[i]>=L'0'&&value[i]<=L'9')||(value[i]>=L'a'&&value[i]<=L'f')))return false;}return wcscmp(value,L"00000000-0000-0000-0000-000000000000")!=0;}
static void toast(const wchar_t* id,bool show=true){
    if(!ticket(id))throw std::runtime_error("Expected a canonical nonempty UUID");
    if(show&&shortcut_target()!=root_path()+L"\\Papo.exe")throw std::runtime_error("Register this package's desktop identity first");
    std::wstring uri=std::wstring(PAPO_PROTOCOL)+L"://open/"+id;
    std::wstring xml=L"<toast activationType=\"protocol\" launch=\""+uri+L"\"><visual><binding template=\"ToastGeneric\"><text>Papo</text><text>Teste de identidade e ativação. Clique para abrir o Papo.</text></binding></visual><audio silent=\"true\"/></toast>";
    Com<IInspectable> instance;check(RoActivateInstance(String(L"Windows.Data.Xml.Dom.XmlDocument"),(IInspectable**)instance.out()));
    Com<IInspectable> document;check(instance->QueryInterface(guid(L"{F7F3A506-1E87-42D6-BCFB-B8C809FA5494}"),(void**)document.out()));Com<XmlIO> io;check(instance->QueryInterface(guid(L"{6CD0E74E-EE65-4489-9EBF-CA43E87BA637}"),(void**)io.out()));check(io->LoadXml(String(xml)));
    Com<ToastFactory> factory;check(RoGetActivationFactory(String(L"Windows.UI.Notifications.ToastNotification"),guid(L"{04124B20-82C6-4229-B109-FD9ED4662B53}"),(void**)factory.out()));
    Com<IInspectable> notification;check(factory->CreateToastNotification(document.p,notification.out()));
    Com<ToastNames> named;check(notification->QueryInterface(guid(L"{9DFB9FD1-143A-490E-90BF-B9FBA7132DE7}"),(void**)named.out()));check(named->put_Tag(String(L"identity-probe")));check(named->put_Group(String(L"papo-step4")));
    Com<ToastManager> manager;check(RoGetActivationFactory(String(L"Windows.UI.Notifications.ToastNotificationManager"),guid(L"{50AC103F-D235-4598-BBEF-98FE4D1A3AD4}"),(void**)manager.out()));
    if(show){Com<ToastNotifier> notifier;check(manager->CreateToastNotifierWithId(String(PAPO_APP_ID),notifier.out()));check(notifier->Show(notification.p));}
}
static void clear_toast(){Com<ToastManager2> manager;check(RoGetActivationFactory(String(L"Windows.UI.Notifications.ToastNotificationManager"),guid(L"{7AB93C52-0E48-4750-BA9D-1A4113981847}"),(void**)manager.out()));Com<ToastHistory> history;check(manager->get_History(history.out()));check(history->RemoveGroupedTagWithId(String(L"identity-probe"),String(L"papo-step4"),String(PAPO_APP_ID)));}
static void inspect(){
    Com<IShellLinkW> link;shell_link(link,true);Com<IPropertyStore> properties;check(link->QueryInterface(IID_PPV_ARGS(properties.out())));
    PROPVARIANT value;PropVariantInit(&value);check(properties->GetValue(PKEY_AppUserModel_ID,&value));bool app=value.vt==VT_LPWSTR&&wcscmp(value.pwszVal,PAPO_APP_ID)==0;PropVariantClear(&value);
    check(properties->GetValue(PKEY_AppUserModel_ToastActivatorCLSID,&value));CLSID expected;check(CLSIDFromString(PAPO_TOAST_CLSID,&expected));bool clsid=value.vt==VT_CLSID&&IsEqualCLSID(*value.puuid,expected);PropVariantClear(&value);
    bool path=shortcut_target()==root_path()+L"\\Papo.exe";
    const auto protocol=keys()[0];
    bool registered=registry(protocol,L"PapoOwner")==PAPO_APP_ID&&registry(protocol+L"\\shell\\open\\command",nullptr)==L"\""+root_path()+L"\\Papo.exe\" --activate \"%1\"";
    if(!app||!clsid||!path||!registered)throw std::runtime_error("Shell identity does not match this package");
    std::printf("{\"app_id\":\"br.com.papo.gtk\",\"shortcut\":true,\"activator\":true,\"target\":true,\"protocol\":true}\n");
}
int wmain(int argc,wchar_t** argv){
    try{check(RoInitialize(RO_INIT_MULTITHREADED));check(SetCurrentProcessExplicitAppUserModelID(PAPO_APP_ID));
        if(argc==2&&wcscmp(argv[1],L"--register-desktop")==0)registration(false);
        else if(argc==2&&wcscmp(argv[1],L"--unregister-desktop")==0)registration(true);
        else if(argc==2&&wcscmp(argv[1],L"--desktop-info")==0)inspect();
        else if(argc==3&&wcscmp(argv[1],L"--test-toast")==0)toast(argv[2]);
        else if(argc==2&&wcscmp(argv[1],L"--clear-test-toast")==0)clear_toast();
        else if(argc==2&&wcscmp(argv[1],L"--self-check")==0){toast(L"70b0d588-40ac-4314-86bf-130ad1c9b2ee",false);std::puts("{\"winrt_abi\":true}");}
        else throw std::runtime_error("Unknown shell command");
        RoUninitialize();return 0;
    }catch(const std::exception& error){std::fprintf(stderr,"Papo desktop: %s\n",error.what());return 1;}
}
