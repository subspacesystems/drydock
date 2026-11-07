# Drydock icon set: 20x20 grid, 1.75 stroke, mitred joins, white ink (tint at runtime).
# Regenerate: python3 generate_icons.py  (Python 3.12+; writes the .svg files next to this script)
import math, os
SW=1.75; INK='#FFFFFF'; RR=2.25  # ring radius
def arcpts(cx,cy,r,a0,a1,n=24):
    return [(cx+r*math.cos(math.radians(a0+(a1-a0)*i/n)), cy+r*math.sin(math.radians(a0+(a1-a0)*i/n))) for i in range(n+1)]
def pl(pts,closed=False):
    d='M'+' L'.join(f'{x:.2f} {y:.2f}' for x,y in pts)
    return d+(' Z' if closed else '')
def circ(cx,cy,r):
    return f'M{cx+r} {cy} A{r} {r} 0 1 1 {cx-r} {cy} A{r} {r} 0 1 1 {cx+r} {cy} Z'
def chevron(tip, frm, size=3.2):
    ux,uy=tip[0]-frm[0],tip[1]-frm[1]; L=math.hypot(ux,uy); ux/=L; uy/=L; px,py=-uy,ux
    b=(tip[0]-ux*size, tip[1]-uy*size)
    return pl([(b[0]+px*size*0.75,b[1]+py*size*0.75), tip, (b[0]-px*size*0.75,b[1]-py*size*0.75)])
def svg(name, strokes=(), fills=(), lights=(), rings=(), dots=(), cuts=(), top=(), raw=''):
    # strokes: path d strings; lights: filled at 35% opacity; rings: (x,y[,r]) ring nodes that knock out lines
    m=''
    if rings or cuts:
        m='<mask id="k" maskUnits="userSpaceOnUse" x="0" y="0" width="20" height="20"><rect width="20" height="20" fill="#fff"/>'+''.join(f'<circle cx="{r[0]}" cy="{r[1]}" r="{(r[2] if len(r)>2 else RR)+1.0:.2f}" fill="#000"/>' for r in rings)+''.join(f'<path d="{d}" fill="none" stroke="#000" stroke-width="{SW+2.5}"/>' for d in cuts)+'</mask>'
    g=''.join(f'<path d="{d}" fill="{INK}" fill-opacity=".35"/>' for d in lights)
    g+=''.join(f'<path d="{d}" fill="{INK}"/>' for d in fills)
    g+=''.join(f'<path d="{d}"/>' for d in strokes)
    g+=''.join(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{INK}" stroke="none"/>' for x,y,r in dots)+raw
    body=f'<g fill="none" stroke="{INK}" stroke-width="{SW}" stroke-linecap="butt" stroke-linejoin="miter" stroke-miterlimit="6"{" mask=\"url(#k)\"" if (rings or cuts) else ""}>{g}</g>'
    body+=''.join(f'<circle cx="{r[0]}" cy="{r[1]}" r="{r[2] if len(r)>2 else RR}" fill="none" stroke="{INK}" stroke-width="1.5"/>' for r in rings)
    body+=''.join(f'<g fill="none" stroke="{INK}" stroke-width="{SW}" stroke-linecap="butt"><path d="{d}"/></g>' for d in top)
    s=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" width="20" height="20">{m}{body}</svg>\n'
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), f'{name}.svg'),'w').write(s)

# --- tools ---
svg('select', strokes=[pl([(5,2.5),(15.5,11),(10.6,11.6),(13.3,17),(11.2,18),(8.6,12.7),(5,15.8)],True)])
svg('line', strokes=[pl([(3,17),(15.5,4.5)])], dots=[(3,17,1.1)], rings=[(15.5,4.5)])
svg('rectangle', strokes=[pl([(15.5,15.5),(15.5,4),(3,4),(3,15.5),(15.5,15.5)])], rings=[(15.5,15.5)])
svg('circle', strokes=[circ(10,10,7.25), pl([(10,10),(15.1,4.9)])], rings=[(10,10)])
svg('arc', strokes=[pl(arcpts(10,15,7.5,180,360)), pl([(2.5,15),(17.5,15)])], rings=[(10,7.5)])
svg('arc3', strokes=[pl(arcpts(9.5,22,10.6,228.7,311.3))], dots=[(2.5,14.05,1.15),(9.5,11.4,1.15)], rings=[(16.5,14.05)])
svg('pushpull', lights=[pl([(2.5,13.5),(10,9.5),(17.5,13.5),(10,17.5)],True)], strokes=[pl([(2.5,13.5),(10,9.5),(17.5,13.5),(10,17.5)],True), pl([(10,13.5),(10,2.5)]), chevron((10,2.2),(10,8))], dots=[(10,13.5,1.4)])
svg('offset', strokes=[pl([(2.5,2.5),(17.5,2.5),(17.5,17.5),(2.5,17.5)],True), pl([(6.5,6.5),(13.5,6.5),(13.5,13.5),(6.5,13.5)],True)], rings=[(13.5,13.5)])
svg('followme', lights=[pl([(2.5,12.5),(7.5,12.5),(7.5,17.5),(2.5,17.5)],True)], strokes=[pl([(2.5,12.5),(7.5,12.5),(7.5,17.5),(2.5,17.5)],True), pl([(5,12.5),(5,4.5),(15.5,4.5)])], rings=[(15.5,4.5)])
mv=[pl([(10,10),(10,2.5)]),pl([(10,10),(10,17.5)]),pl([(10,10),(2.5,10)]),pl([(10,10),(17.5,10)]),chevron((10,1.9),(10,8)),chevron((10,18.1),(10,12)),chevron((1.9,10),(8,10)),chevron((18.1,10),(12,10))]
svg('move', strokes=mv, rings=[(10,10)])
rp=arcpts(10,10,7,-60,210,30)
svg('rotate', strokes=[pl(rp), chevron(rp[0], rp[3], 3.0)], rings=[(10,10)])
svg('scale', strokes=[pl([(14.5,2.5),(2.5,2.5),(2.5,17.5),(17.5,17.5),(17.5,5.5)]), pl([(6,14),(14.5,5.5)]), chevron((5.6,14.4),(9,11),2.8)], fills=[pl([(2.5,13),(7,13),(7,17.5),(2.5,17.5)],True)], rings=[(17,3)])
svg('tape', strokes=[pl([(2.5,5),(2.5,15)]), pl([(17.5,5),(17.5,15)]), pl([(3.5,10),(16.5,10)]), chevron((3.4,10),(8,10),2.8), chevron((16.6,10),(12,10),2.8)])
def rot(p,a,c):
    s,co=math.sin(a),math.cos(a); x,y=p[0]-c[0],p[1]-c[1]; return (c[0]+x*co-y*s, c[1]+x*s+y*co)
a=math.radians(-30); c=(10,9)
E=[rot(p,a,c) for p in [(3,6),(17,6),(17,12),(3,12)]]; H=[rot(p,a,c) for p in [(3,6),(9,6),(9,12),(3,12)]]
svg('erase', strokes=[pl(E,True), pl([H[1],H[2]]), pl([(3,17.5),(17,17.5)])], fills=[pl(H,True)])
# paint bucket, pouring to the left (flipped left-to-right)
b=[rot(p,math.radians(-25),(11,11)) for p in [(6,7),(16,7),(14.5,16),(7.5,16)]]
hd=[rot(p,math.radians(-25),(11,11)) for p in arcpts(11,7,4.2,180,360,14)]
svg('paint', lights=[pl(b,True)], strokes=[pl(b,True), pl(hd)], fills=['M3.6 9.5 L5.4 12.6 A1.9 1.9 0 1 1 1.9 12.6 Z'])
op=[rot((10+7.5*math.cos(t),10+3.2*math.sin(t)),math.radians(-25),(10,10)) for t in [2*math.pi*i/40 for i in range(41)]]
svg('orbit', strokes=[pl(op[4:40]), chevron(op[40],op[37],3.0)], dots=[(10,10,1.4)])
svg('pan', strokes=['M6 10.5 V4.5 Q6 3 7.25 3 Q8.5 3 8.5 4.5 V9.5 M8.5 4 V2.75 Q8.5 1.5 9.75 1.5 Q11 1.5 11 2.75 V9.5 M11 3.5 Q11 2.25 12.25 2.25 Q13.5 2.25 13.5 3.5 V9.5 M13.5 5.5 Q13.5 4.25 14.75 4.25 Q16 4.25 16 5.5 V12.5 Q16 18.5 10.5 18.5 Q7.5 18.5 5.5 16 L2.5 12 Q1.6 10.7 2.8 10 Q4 9.4 5 10.6 L6 11.8'])
svg('zoom', strokes=[circ(8.25,8.25,5.75), pl([(12.4,12.4),(17.8,17.8)])])
# --- panel icons ---
svg('list', strokes=[pl([(7,4.5),(17.5,4.5)]),pl([(7,10),(17.5,10)]),pl([(7,15.5),(17.5,15.5)])], dots=[(3.5,4.5,1.3),(3.5,10,1.3),(3.5,15.5,1.3)])
svg('tiles', strokes=[pl([(2.5,2.5),(8.5,2.5),(8.5,8.5),(2.5,8.5)],True),pl([(11.5,2.5),(17.5,2.5),(17.5,8.5),(11.5,8.5)],True),pl([(2.5,11.5),(8.5,11.5),(8.5,17.5),(2.5,17.5)],True),pl([(11.5,11.5),(17.5,11.5),(17.5,17.5),(11.5,17.5)],True)])
svg('new', strokes=[pl([(10,3),(10,17)]),pl([(3,10),(17,10)])])
P=[(13.5,2.5),(17.5,6.5),(7,17),(2.5,17.5),(3,13)]
svg('edit', strokes=[pl(P,True), pl([(11,5),(15,9)])])
svg('purge', strokes=[pl([(2.5,5),(17.5,5)]), pl([(7.5,5),(7.5,2.5),(12.5,2.5),(12.5,5)]), pl([(4.5,7.5),(5.5,17.5),(14.5,17.5),(15.5,7.5)]), pl([(8.25,9),(8.5,15)]), pl([(11.75,9),(11.5,15)])])
svg('eyedropper', fills=['M13.2 2.6 A3 3 0 0 1 17.4 6.8 L15.6 8.6 L11.4 4.4 Z'], strokes=[pl([(10.4,5.4),(14.6,9.6)]), pl([(12.4,7.6),(4.6,15.4),(3,17),(4.6,15.4)]), pl([(12.4,7.6),(5,15),(2.5,17.5)])])
# eye: almond from two arcs meeting in mitred corners at x=2 and x=18; iris is the ring node (no pupil: at 20 px it only clutters)
EYE='M2 10 A8.9 8.9 0 0 1 18 10 A8.9 8.9 0 0 1 2 10 Z'
IRIS='<circle cx="10" cy="10" r="2.5" stroke-width="1.5"/>'
svg('eye', strokes=[EYE], rings=[(10,10,2.5)])
svg('eye-off', strokes=[EYE], cuts=[pl([(3,17),(17,3)])], raw=IRIS, top=[pl([(3,17),(17,3)])])
print('wrote', len([f for f in os.listdir(os.path.dirname(os.path.abspath(__file__))) if f.endswith('.svg')]), 'icons')
