#!/usr/bin/env python3
"""Verify native audio buttons and generated PCM using temporary virtual devices only.
Requires Linux, pactl, gst-launch-1.0, Podman and the existing papo-ci:fedora44 image.
"""
from pathlib import Path
import subprocess,os,uuid,signal,tempfile,array,math,json,argparse,sys
if not sys.platform.startswith('linux'):raise SystemExit('Linux virtual audio fixture required')
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary',type=Path,required=True,help='Current compiled Rust test executable')
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
binary=args.binary.resolve()
if not binary.is_file() or not binary.is_relative_to(root):raise SystemExit('Test executable must exist inside this checkout')
relative_binary=str(binary.relative_to(root))
os.chdir(root)
base='papo_audio_test_'+uuid.uuid4().hex[:8];sink=base+'_output';input_sink=base+'_input';source=base+'_capture';modules=[];process=None;recorder=None
try:
 for name in [sink,input_sink]:modules.append(subprocess.check_output(['pactl','load-module','module-null-sink','sink_name='+name,'sink_properties=device.description=PapoAudioRegression node.driver=true'],text=True).strip())
 modules.append(subprocess.check_output(['pactl','load-module','module-remap-source','master='+input_sink+'.monitor','source_name='+source,'source_properties=device.description=PapoAudioRegressionInput'],text=True).strip())
 with tempfile.TemporaryDirectory(prefix='papo-audio-functional-') as temp:
  pcm=Path(temp)/'generated-output.raw';rec_log=open(Path(temp)/'recorder.log','w')
  recorder=subprocess.Popen(['gst-launch-1.0','-q','pulsesrc','device='+sink+'.monitor','client-name=PapoAudioVerification','!','audioconvert','!','audioresample','!','audio/x-raw,format=S16LE,rate=48000,channels=1','!','filesink','location='+str(pcm)],stdout=subprocess.DEVNULL,stderr=rec_log,start_new_session=True)
  pulse=Path(os.environ.get('XDG_RUNTIME_DIR','/run/user/'+str(os.getuid())))/'pulse'
  cmd=['podman','run','--rm','--pull=never','--userns=keep-id','--cap-drop','ALL','--security-opt','label=disable','--security-opt','seccomp=unconfined','--security-opt','apparmor=unconfined','--security-opt','unmask=/proc/*','--volume',str(root)+':/work:rw','--volume',str(pulse)+':/tmp/papo-pulse:rw','--workdir','/work']
  env={'PULSE_SERVER':'unix:/tmp/papo-pulse/native','PULSE_SINK':sink,'PULSE_SOURCE':source,'PAPO_AUDIO_NATIVE_TEST_SINK':sink,'PAPO_AUDIO_NATIVE_TEST_SOURCE':source,'PAPO_AUDIO_NATIVE_INPUT_SINK':input_sink,'GST_PLUGIN_FEATURE_RANK':'pulsesrc:4096,pulsesink:4096,pipewiresrc:0,alsasrc:0,osssrc:0,oss4src:0','GSK_RENDERER':'cairo','GTK_A11Y':'none','G_DEBUG':'fatal-criticals','PAPO_TEST_HELPERS_DIR':'/work/tests/helpers','XDG_CONFIG_HOME':'/tmp/papo-audio-real-config','XDG_CACHE_HOME':'/tmp/papo-audio-real-cache'}
  for k,v in env.items():cmd+=['--env',k+'='+v]
  cmd+=['localhost/papo-ci:fedora44','bash','target/steps1617-wm.sh','bash','-c',f'gst-launch-1.0 -q audiotestsrc is-live=true volume=0.2 freq=880 ! audioconvert ! audioresample ! pulsesink "device=$PAPO_AUDIO_NATIVE_INPUT_SINK" "stream-properties=props,application.name=PapoAudioFixture,application.id=br.com.papo.gtk.AudioFixture" >/tmp/tone.log 2>&1 & tone=$!; trap \'kill "$tone" 2>/dev/null || true\' EXIT; python3 scripts/run-gui-case.py audio-setup --binary {relative_binary} --output-dir target/audio-device-tests --timeout 60']
  process=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,start_new_session=True)
  try:output,_=process.communicate(timeout=85)
  except subprocess.TimeoutExpired:
   os.killpg(process.pid,signal.SIGKILL);output,_=process.communicate();print(output[-9000:]);raise
  print(output[-9000:])
  os.killpg(recorder.pid,signal.SIGINT)
  try:recorder.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(recorder.pid,signal.SIGKILL);recorder.wait()
  rec_log.close()
  samples=array.array('h');data=pcm.read_bytes() if pcm.exists() else b'';samples.frombytes(data[:len(data)//2*2])
  result={'status':'fail','gtk_exit_code':process.returncode,'captured_samples':len(samples),'peak':max(map(abs,samples),default=0)/32768.0,'frequencies':{}}
  size=4800
  for frequency in [880,660]:
   cosine=[math.cos(2*math.pi*frequency*i/48000) for i in range(size)];sine=[math.sin(2*math.pi*frequency*i/48000) for i in range(size)];windows=0
   for start in range(0,len(samples)-size+1,size):
    part=samples[start:start+size]
    a=sum(v*c for v,c in zip(part,cosine));b=sum(v*s for v,s in zip(part,sine));amplitude=2*math.hypot(a,b)/(size*32768)
    if amplitude>0.003:windows+=1
   result['frequencies'][str(frequency)]={'audible_seconds':windows/10.0}
  good=process.returncode==0 and result['frequencies']['880']['audible_seconds']>=1.0 and result['frequencies']['660']['audible_seconds']>=1.0
  result['status']='pass' if good else 'fail';out=root/'target/audio-device-tests';out.mkdir(exist_ok=True);(out/'pcm-verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));assert good,'The buttons must produce recorded/replayed 880 Hz audio and a real 660 Hz output tone'
finally:
 for p in [process,recorder]:
  if p and p.poll() is None:os.killpg(p.pid,signal.SIGKILL);p.wait()
 for module in reversed(modules):subprocess.run(['pactl','unload-module',module],check=True)
