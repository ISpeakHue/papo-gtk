// EN-US: Explicit synthetic native-API diagnostic; never qualifies a package or reads saved sessions.
// PT-BR: Diagnóstico nativo sintético explícito; não qualifica pacotes nem lê sessões salvas.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <objbase.h>
#include <cstdio>
#include <io.h>
#include <fcntl.h>
#include <fstream>
#include <filesystem>
#include <string>
extern "C" DWORD papo_media_run(const wchar_t*,const wchar_t*,const wchar_t*,const wchar_t*,HANDLE,DWORD,void(*)(void*,int,const unsigned char*,size_t),void*,DWORD*) noexcept;
static void sink(void*,int channel,const unsigned char* data,size_t size){fwrite(data,1,size,channel==1?stdout:stderr);}
int wmain(int argc,wchar_t**argv){
 if(argc!=4)return 2;
 _setmode(_fileno(stdout),_O_BINARY);
 std::ifstream file(std::filesystem::path(argv[3]),std::ios::binary);std::string bytes((std::istreambuf_iterator<char>(file)),std::istreambuf_iterator<char>());
 int length=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,bytes.data(),int(bytes.size()),nullptr,0);if(!length)return 3;
 std::wstring script(length,0);MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,bytes.data(),int(bytes.size()),script.data(),length);
 GUID id{};CoCreateGuid(&id);wchar_t guid[40];StringFromGUID2(id,guid,40);std::wstring uid(guid+1,36);for(auto&c:uid)if(c>=L'A'&&c<=L'F')c+=32;
 std::wstring name=L"Papo.Media."+std::to_wstring(GetCurrentProcessId())+L"."+uid;
 HANDLE event=CreateEventW(nullptr,TRUE,FALSE,nullptr);if(!event)return 4;DWORD code=1;DWORD result=papo_media_run(argv[1],argv[2],name.c_str(),script.c_str(),event,25000,sink,nullptr,&code);CloseHandle(event);
 fprintf(stderr,"\nNative sandbox result=%lu child=%lu\n",result,code);return result?1:int(code);
}
