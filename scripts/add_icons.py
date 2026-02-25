#!/usr/bin/env python3
import re
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen

FONT = Path("C:/Users/max/Documents/Development/orderflow/assets/fonts/icons.ttf")
SVGS = Path("C:/Users/max/Documents/Development/orderflow/assets/fonts/feathericons")

NEW_ICONS = [
    (0xE837, "message-square", "message-square.svg"),
    (0xE838, "send",           "send.svg"),
    (0xE839, "cpu",            "cpu.svg"),
    (0xE83A, "refresh-cw",     "refresh-cw.svg"),
]

SCALE = 38.0
X_OFF = 44.0
Y_TOP = 812.0

def svg_to_font_pt(x, y):
    return (SCALE * x + X_OFF, -SCALE * y + Y_TOP)

_TOKEN_RE = re.compile(r'[MmLlHhVvCcSsQqTtAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?')

def tokenize(d):
    return _TOKEN_RE.findall(d)

def draw_path_to_tt_pen(d, pen):
    toks = tokenize(d)
    n = len(toks)
    i = 0
    cx = cy = 0.0
    sx = sy = 0.0
    px2 = py2 = 0.0
    last_cmd = ""
    in_path = False

    def nums(count):
        nonlocal i
        vals = [float(toks[i + k]) for k in range(count)]
        i += count
        return vals

    def pt(x, y):
        return svg_to_font_pt(x, y)

    while i < n:
        t = toks[i]
        if re.match(r'^[MmLlHhVvCcSsQqTtAaZz]$', t):
            cmd = t
            i += 1
        else:
            cmd = 'L' if last_cmd == 'M' else ('l' if last_cmd == 'm' else last_cmd)

        last_cmd = cmd

        if cmd not in ('S', 's', 'C', 'c', 'Q', 'q', 'T', 't'):
            px2, py2 = cx, cy

        if cmd == 'M':
            if in_path:
                pen.endPath()
                in_path = False
            cx, cy = nums(2)
            sx, sy = cx, cy
            pen.moveTo(pt(cx, cy))
            in_path = True
        elif cmd == 'm':
            if in_path:
                pen.endPath()
                in_path = False
            dx, dy = nums(2)
            cx += dx; cy += dy
            sx, sy = cx, cy
            pen.moveTo(pt(cx, cy))
            in_path = True
        elif cmd == 'L':
            x, y = nums(2); cx, cy = x, y
            pen.lineTo(pt(cx, cy))
        elif cmd == 'l':
            dx, dy = nums(2); cx += dx; cy += dy
            pen.lineTo(pt(cx, cy))
        elif cmd == 'H':
            cx = nums(1)[0]; pen.lineTo(pt(cx, cy))
        elif cmd == 'h':
            cx += nums(1)[0]; pen.lineTo(pt(cx, cy))
        elif cmd == 'V':
            cy = nums(1)[0]; pen.lineTo(pt(cx, cy))
        elif cmd == 'v':
            cy += nums(1)[0]; pen.lineTo(pt(cx, cy))
        elif cmd == 'C':
            x1,y1,x2,y2,x,y = nums(6)
            px2,py2 = x2,y2
            pen.curveTo(pt(x1,y1), pt(x2,y2), pt(x,y))
            cx,cy = x,y
        elif cmd == 'c':
            x1,y1,x2,y2,dx,dy = nums(6)
            x1+=cx; y1+=cy; x2+=cx; y2+=cy; x=cx+dx; y=cy+dy
            px2,py2 = x2,y2
            pen.curveTo(pt(x1,y1), pt(x2,y2), pt(x,y))
            cx,cy = x,y
        elif cmd == 'Q':
            x1,y1,x,y = nums(4); px2,py2=x1,y1
            pen.qCurveTo(pt(x1,y1), pt(x,y)); cx,cy=x,y
        elif cmd == 'q':
            x1,y1,dx,dy = nums(4)
            x1+=cx; y1+=cy; x=cx+dx; y=cy+dy; px2,py2=x1,y1
            pen.qCurveTo(pt(x1,y1), pt(x,y)); cx,cy=x,y
        elif cmd == 'S':
            x2,y2,x,y = nums(4)
            x1=2*cx-px2; y1=2*cy-py2; px2,py2=x2,y2
            pen.curveTo(pt(x1,y1), pt(x2,y2), pt(x,y)); cx,cy=x,y
        elif cmd == 's':
            x2,y2,dx,dy = nums(4)
            x1=2*cx-px2; y1=2*cy-py2
            x2+=cx; y2+=cy; x=cx+dx; y=cy+dy; px2,py2=x2,y2
            pen.curveTo(pt(x1,y1), pt(x2,y2), pt(x,y)); cx,cy=x,y
        elif cmd == 'T':
            x,y = nums(2); x1=2*cx-px2; y1=2*cy-py2; px2,py2=x1,y1
            pen.qCurveTo(pt(x1,y1), pt(x,y)); cx,cy=x,y
        elif cmd == 't':
            dx,dy = nums(2); x1=2*cx-px2; y1=2*cy-py2
            x=cx+dx; y=cy+dy; px2,py2=x1,y1
            pen.qCurveTo(pt(x1,y1), pt(x,y)); cx,cy=x,y
        elif cmd in ('Z', 'z'):
            pen.closePath(); cx,cy=sx,sy; in_path=False
        elif cmd in ('A', 'a'):
            rx,ry,xrot,large,sweep,x,y = nums(7)
            if cmd == 'a': x+=cx; y+=cy
            pen.lineTo(pt(x,y)); cx,cy=x,y

    if in_path:
        pen.endPath()

def expand_strokes(svg_path):
    from picosvg.svg import SVG
    svg = SVG.parse(str(svg_path))
    try:
        return svg.topicosvg()
    except Exception as e:
        print(f"  WARNING: picosvg failed ({e}), using raw SVG")
        return svg

def get_path_ds(svg_obj):
    NS = "http://www.w3.org/2000/svg"
    root = svg_obj.toetree()  # toetree() returns the root element directly
    paths = []
    for el in root.iter("{" + NS + "}path"):
        d = el.get("d", "").strip()
        if d:
            paths.append(d)
    return paths

def build_glyph(path_ds, font):
    pen = TTGlyphPen(font.getReverseGlyphMap())
    for d in path_ds:
        draw_path_to_tt_pen(d, pen)
    return pen.glyph()

def main():
    print(f"Loading font: {FONT}")
    font = TTFont(str(FONT))

    glyph_order = list(font.getGlyphOrder())
    cmap_table = font["cmap"]
    hmtx = font["hmtx"]

    # Collect all Unicode cmap subtables to keep them in sync
    unicode_cmaps = [sub for sub in cmap_table.tables if sub.isUnicode() and hasattr(sub, "cmap")]
    if not unicode_cmaps:
        raise RuntimeError("No Unicode cmap subtable found")
    print(f"Found {len(unicode_cmaps)} Unicode cmap subtable(s)")

    for cp, name, fname in NEW_ICONS:
        svg_path = SVGS / fname
        print(); print(f"Processing U+{cp:04X} '{name}' from {fname}")
        pico = expand_strokes(svg_path)
        paths = get_path_ds(pico)
        print(f"  Got {len(paths)} path(s)")
        glyph = build_glyph(paths, font)
        font["glyf"][name] = glyph
        if name not in glyph_order:
            glyph_order.append(name)
        lsb = glyph.xMin if hasattr(glyph, "xMin") and glyph.xMin is not None else 0
        hmtx.metrics[name] = (1000, lsb)
        # Update ALL Unicode subtables (format 4, 12, etc.)
        for sub in unicode_cmaps:
            sub.cmap[cp] = name
        print(f"  Inserted '{name}' -> U+{cp:04X}")

    font.setGlyphOrder(glyph_order)
    print(); print(f"Saving to: {FONT}")
    font.save(str(FONT))
    print("Saved.")

    print(); print("--- Verification ---")
    font2 = TTFont(str(FONT))
    cmap2 = font2.getBestCmap()
    for cp, name, _ in NEW_ICONS:
        found = cmap2.get(cp)
        status = "OK" if found == name else f"MISSING (got {found!r})"
        print(f"  U+{cp:04X} -> '{name}': {status}")
    all_cp = sorted(cmap2.keys())
    print(f"Total codepoints: {len(all_cp)}")
    print(f"Range: U+{all_cp[0]:04X} - U+{all_cp[-1]:04X}")
    new_cp = [hex(c) for c in all_cp if c >= 0xE837]
    print(f"New codepoints: {new_cp}")

if __name__ == "__main__":
    main()
