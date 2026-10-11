"""Static canonicalization of current floating Controller SVG hit regions."""
import json
from pathlib import Path
import math
import re
from keyboard_geometry import path_commands

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / 'crates/razer-pages/src/features/keyboard_floating_controller_data.json'
records = json.loads(path.read_text(encoding='utf8'))
for record in records:
    for button in record['buttons']:
        raw = button['shape']['data']
        match = re.fullmatch(r'translate\(([-.\d]+) ([-.\d]+)\)',raw['transform'])
        if not match: raise ValueError('Unaudited transform '+raw['transform'])
        dx,dy = map(float,match.groups())
        if button['shape']['type'] == 'circle':
            center = [raw['cx']+dx,raw['cy']+dy]
            radius = raw['r']
            bounds = [center[0]-radius, center[1]-radius, radius*2, radius*2]
            geometry = {'kind':'circle', 'center':center, 'radius':radius}
        else:
            commands = path_commands(raw['d'])
            samples=[]
            previous=[0,0]
            for command in commands:
                if command['op']=='close': continue
                p=command['points']
                if command['op']=='curve':
                    points=[previous,*p]
                    ts={0.,1.}
                    for axis in (0,1):
                        a,b,c,d=[q[axis] for q in points]
                        qa=-a+3*b-3*c+d; qb=2*(a-2*b+c); qc=b-a
                        if abs(qa)<1e-12:
                            if abs(qb)>1e-12: ts.add(-qc/qb)
                        elif qb*qb-4*qa*qc>=0:
                            root=math.sqrt(qb*qb-4*qa*qc)
                            ts.update(((-qb-root)/(2*qa),(-qb+root)/(2*qa)))
                    for t in ts:
                        if 0<=t<=1: samples.append([sum(points[i][axis]*math.comb(3,i)*t**i*(1-t)**(3-i) for i in range(4)) for axis in (0,1)])
                    previous=p[-1]
                else: samples.append(p);previous=p
            x,y=[min(p[axis] for p in samples) for axis in (0,1)]
            right,bottom=[max(p[axis] for p in samples) for axis in (0,1)]
            x+=dx;right+=dx;y+=dy;bottom+=dy
            for command in commands:
                if command['op']=='close':continue
                if command['op']=='curve':command['points']=[[p[0]+dx,p[1]+dy] for p in command['points']]
                else:command['points']=[command['points'][0]+dx,command['points'][1]+dy]
            bounds = [x,y,right-x,bottom-y]
            geometry = {'kind':'path','commands':commands}
        button['key'] = {'id':button['id'],'label':button['name'],'enabled':True,
                         'functions':[], 'bounds':bounds,'geometry':geometry}
path.write_text(json.dumps(records, separators=(',', ':')), encoding='utf8')
print('Prepared 72 original controller hit regions')
