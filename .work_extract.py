from pathlib import Path
for pid,name in [(3858,'.ref/devices/3858/static/js/main.608e1599.js'),(3880,'.ref/devices/3880/static/js/main.0ff487d9.js')]:
 s=Path(name).read_text(encoding='utf-8')
 print('PID',pid,'len',len(s))
 for term in ['colorProfiles','PERFORMANCE_MODE_SCREEN_COLOR_PROFILE','refeshRateCounter','screen-refresh-container','refreshRate']:
  pos=0;n=0
  while True:
   pos=s.find(term,pos)
   if pos<0 or n>=10: break
   print('\nTERM',term,'POS',pos,'\n',s[max(0,pos-1000):pos+2000])
   pos+=len(term);n+=1
