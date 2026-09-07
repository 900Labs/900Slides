#!/usr/bin/env python3
"""Generate original, redistributable PPTX smoke workloads using only stdlib.

Usage: python3 scripts/generate-perf-fixtures.py --output-dir /tmp/900slides-fixtures
Add --workload all for classroom/standard/stress; --include-negative adds three
separate failure/preservation cases. Existing output files are never replaced.

These fixtures use synthetic RGB PNGs, not the JPEG photos specified by
docs/LOW_RESOURCE_REQUIREMENTS.md. They exercise the requested pixel dimensions
but compress unusually well, so are smoke workloads, not hardware qualification.
No external assets, account, network, fonts, timestamps or personal data are used.
The generator and its original content use the repository's Apache-2.0 license.
"""

import argparse
import hashlib
import json
import posixpath
import struct
import sys
import xml.etree.ElementTree as ET
import zlib
from pathlib import Path
from xml.sax.saxutils import escape
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo


P = "http://schemas.openxmlformats.org/presentationml/2006/main"
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
C = "http://schemas.openxmlformats.org/drawingml/2006/chart"
REL = "http://schemas.openxmlformats.org/package/2006/relationships"
CT = "application/vnd.openxmlformats-officedocument."
NS = f'xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}"'
XML = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
WIDTH, HEIGHT = 12192000, 6858000
PROFILES = {
    "classroom": (20, 10, 1280, 720, 10 * 1024 * 1024),
    "standard": (50, 20, 1920, 1080, 25 * 1024 * 1024),
    "stress": (200, 100, 1280, 720, None),
}
GROUP = ('<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/>'
         '<p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm>'
         '<a:off x="0" y="0"/><a:ext cx="0" cy="0"/>'
         '<a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/>'
         '</a:xfrm></p:grpSpPr>')


def rels(entries):
    return XML + f'<Relationships xmlns="{REL}">' + ''.join(
        f'<Relationship Id="{rid}" Type="{kind}" Target="{target}"/>'
        for rid, kind, target in entries
    ) + '</Relationships>'


def transform(x, y, width, height, tag="a:xfrm"):
    return (f'<{tag}><a:off x="{x}" y="{y}"/>'
            f'<a:ext cx="{width}" cy="{height}"/></{tag}>')


def paragraph(text, size=2000, color="263C4A", bold=False):
    return ('<a:p><a:pPr/><a:r>'
            f'<a:rPr lang="en-US" sz="{size}" b="{int(bold)}">'
            f'<a:solidFill><a:srgbClr val="{color}"/></a:solidFill>'
            '<a:latin typeface="Liberation Sans"/></a:rPr>'
            f'<a:t>{escape(text)}</a:t></a:r><a:endParaRPr lang="en-US"/></a:p>')


def textbox(shape_id, text, rect, size=2000, bold=False, notes=False):
    placeholder = '<p:ph type="body" idx="1"/>' if notes else ''
    return (f'<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="Text {shape_id}"/>'
            f'<p:cNvSpPr txBox="1"/><p:nvPr>{placeholder}</p:nvPr></p:nvSpPr>'
            '<p:spPr>' + transform(*rect) + '<a:prstGeom prst="rect"><a:avLst/>'
            '</a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr>'
            '<p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>'
            + ''.join(paragraph(line, size, bold=bold) for line in text.split('\n'))
            + '</p:txBody></p:sp>')


def rectangle(shape_id, rect, color):
    return (f'<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="Accent {shape_id}"/>'
            '<p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>' + transform(*rect)
            + '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>'
            f'<a:solidFill><a:srgbClr val="{color}"/></a:solidFill>'
            '<a:ln><a:noFill/></a:ln></p:spPr></p:sp>')


def picture(shape_id, rect):
    return (f'<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="Generated mosaic" '
            'descr="Original synthetic color mosaic; no source photograph"/>'
            '<p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>'
            '<p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rImage"/>'
            '<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>'
            + transform(*rect) + '<a:prstGeom prst="rect"><a:avLst/>'
            '</a:prstGeom></p:spPr></p:pic>')


def frame(shape_id, name, uri, contents):
    return (f'<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{shape_id}" '
            f'name="{name}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>'
            + transform(6400000, 2000000, 5000000, 3600000, "p:xfrm")
            + f'<a:graphic><a:graphicData uri="{uri}">{contents}</a:graphicData>'
            '</a:graphic></p:graphicFrame>')


def table():
    rows = []
    for row in range(10):
        cells = []
        for col in range(5):
            text = f'Week {col + 1}' if row == 0 else str((row + 2) * (col + 1))
            cells.append('<a:tc><a:txBody><a:bodyPr/><a:lstStyle/>'
                         + paragraph(text, 1100, bold=row == 0)
                         + '</a:txBody><a:tcPr/></a:tc>')
        rows.append('<a:tr h="360000">' + ''.join(cells) + '</a:tr>')
    content = ('<a:tbl><a:tblPr firstRow="1" bandRow="1"/><a:tblGrid>'
               + '<a:gridCol w="1000000"/>' * 5 + '</a:tblGrid>'
               + ''.join(rows) + '</a:tbl>')
    return frame(6, "Generated 10 by 5 table", A.rsplit('/', 1)[0] + '/table', content)


def chart():
    categories = ''.join(f'<c:pt idx="{i}"><c:v>{name}</c:v></c:pt>'
                         for i, name in enumerate(["Prepare", "Practice", "Share"]))
    values = ''.join(f'<c:pt idx="{i}"><c:v>{value}</c:v></c:pt>'
                     for i, value in enumerate([12, 18, 24]))
    return (XML + f'<c:chartSpace xmlns:c="{C}" xmlns:a="{A}" xmlns:r="{R}">'
            '<c:chart><c:plotArea><c:layout/><c:barChart><c:barDir val="col"/>'
            '<c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:order val="0"/>'
            '<c:tx><c:v>Invented lesson counts</c:v></c:tx><c:cat><c:strLit>'
            f'<c:ptCount val="3"/>{categories}</c:strLit></c:cat><c:val><c:numLit>'
            f'<c:formatCode>0</c:formatCode><c:ptCount val="3"/>{values}'
            '</c:numLit></c:val></c:ser><c:axId val="1"/><c:axId val="2"/>'
            '</c:barChart><c:catAx><c:axId val="1"/><c:scaling>'
            '<c:orientation val="minMax"/></c:scaling><c:axPos val="b"/>'
            '<c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/>'
            '<c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="l"/>'
            '<c:crossAx val="1"/></c:valAx></c:plotArea><c:plotVisOnly val="1"/>'
            '</c:chart></c:chartSpace>')


def png(width, height, seed):
    """Stream 32-pixel mosaic rows into a deterministic RGB PNG compressor."""
    compressor = zlib.compressobj(level=9)
    compressed = []
    for top in range(0, height, 32):
        cells = []
        for left in range(0, width, 32):
            # Integer-only original pattern. Every seed produces distinct media.
            red = 40 + (left // 32 * 13 + top // 32 * 7 + seed * 11) % 180
            green = 45 + (left // 32 * 5 + top // 32 * 17 + seed * 23) % 170
            blue = 60 + (left // 32 * 19 + top // 32 * 3 + seed * 31) % 160
            cells.append(bytes((red, green, blue)) * min(32, width - left))
        row = b'\x00' + b''.join(cells)
        compressed.append(compressor.compress(row * min(32, height - top)))
    compressed.append(compressor.flush())

    def chunk(kind, data):
        return (struct.pack('>I', len(data)) + kind + data
                + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff))

    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', b''.join(compressed)) + chunk(b'IEND', b''))


def theme():
    colors = {"dk1": "263C4A", "lt1": "FFFFFF", "dk2": "355261", "lt2": "F5F7F9",
              "accent1": "197568", "accent2": "E9B45B", "accent3": "527D92",
              "accent4": "987CA0", "accent5": "799862", "accent6": "BA7561",
              "hlink": "176AAB", "folHlink": "78589A"}
    fills = '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>' * 3
    lines = ('<a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/>'
             '</a:solidFill><a:prstDash val="solid"/></a:ln>') * 3
    return (XML + f'<a:theme xmlns:a="{A}" name="Generated classroom">'
            '<a:themeElements><a:clrScheme name="Classroom">'
            + ''.join(f'<a:{key}><a:srgbClr val="{value}"/></a:{key}>'
                      for key, value in colors.items())
            + '</a:clrScheme><a:fontScheme name="Local sans">'
            + ''.join(f'<a:{kind}Font><a:latin typeface="Liberation Sans"/>'
                      f'<a:ea typeface=""/><a:cs typeface=""/></a:{kind}Font>'
                      for kind in ('major', 'minor'))
            + '</a:fontScheme><a:fmtScheme name="Simple">'
            f'<a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst>'
            '<a:effectStyleLst>' + '<a:effectStyle><a:effectLst/></a:effectStyle>' * 3
            + f'</a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst>'
            '</a:fmtScheme></a:themeElements></a:theme>')


def build_parts(workload):
    count, image_count, image_width, image_height, _ = PROFILES[workload]
    parts = {}
    overrides = []

    def add(name, data, content_type=None):
        parts[name] = data.encode('utf-8') if isinstance(data, str) else data
        if content_type:
            overrides.append((name, content_type))

    def presentation_part(name, data, kind):
        add(name, data, CT + f'presentationml.{kind}+xml')

    presentation_part('ppt/presentation.xml', XML + f'<p:presentation {NS}>'
                      '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rMaster"/>'
                      '</p:sldMasterIdLst><p:notesMasterIdLst>'
                      '<p:notesMasterId r:id="rNotesMaster"/></p:notesMasterIdLst><p:sldIdLst>'
                      + ''.join(f'<p:sldId id="{255 + i}" r:id="rSlide{i}"/>'
                                for i in range(1, count + 1))
                      + f'</p:sldIdLst><p:sldSz cx="{WIDTH}" cy="{HEIGHT}" type="screen16x9"/>'
                      '<p:notesSz cx="6858000" cy="9144000"/></p:presentation>',
                      'presentation.main')
    add('_rels/.rels', rels([('rOffice', R + '/officeDocument', 'ppt/presentation.xml')]))
    add('ppt/_rels/presentation.xml.rels', rels(
        [('rMaster', R + '/slideMaster', 'slideMasters/slideMaster1.xml'),
         ('rNotesMaster', R + '/notesMaster', 'notesMasters/notesMaster1.xml'),
         ('rTheme', R + '/theme', 'theme/theme1.xml')]
        + [(f'rSlide{i}', R + '/slide', f'slides/slide{i}.xml') for i in range(1, count + 1)]))
    colors = ('<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" '
              'accent1="accent1" accent2="accent2" accent3="accent3" '
              'accent4="accent4" accent5="accent5" accent6="accent6" '
              'hlink="hlink" folHlink="folHlink"/>')
    presentation_part('ppt/slideMasters/slideMaster1.xml', XML + f'<p:sldMaster {NS}>'
                      f'<p:cSld><p:spTree>{GROUP}</p:spTree></p:cSld>{colors}'
                      '<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rLayout"/>'
                      '</p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/>'
                      '<p:otherStyle/></p:txStyles></p:sldMaster>', 'slideMaster')
    add('ppt/slideMasters/_rels/slideMaster1.xml.rels', rels([
        ('rLayout', R + '/slideLayout', '../slideLayouts/slideLayout1.xml'),
        ('rTheme', R + '/theme', '../theme/theme1.xml')]))
    presentation_part('ppt/slideLayouts/slideLayout1.xml', XML + f'<p:sldLayout {NS} '
                      f'type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>{GROUP}'
                      '</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/>'
                      '</p:clrMapOvr></p:sldLayout>', 'slideLayout')
    add('ppt/slideLayouts/_rels/slideLayout1.xml.rels', rels([
        ('rMaster', R + '/slideMaster', '../slideMasters/slideMaster1.xml')]))
    presentation_part('ppt/notesMasters/notesMaster1.xml', XML + f'<p:notesMaster {NS}>'
                      f'<p:cSld><p:spTree>{GROUP}</p:spTree></p:cSld>{colors}'
                      '<p:notesStyle/></p:notesMaster>', 'notesMaster')
    add('ppt/notesMasters/_rels/notesMaster1.xml.rels', rels([
        ('rTheme', R + '/theme', '../theme/theme1.xml')]))
    add('ppt/theme/theme1.xml', theme(), CT + 'theme+xml')

    for i in range(1, image_count + 1):
        add(f'ppt/media/image{i}.png', png(image_width, image_height, i))
    for i in range(1, count + 1):
        title = f'{workload.title()} lesson | {i:03d}'
        text = (f'Observe the pattern in example {i}.\n'
                'Describe one change and explain your reasoning.\n'
                'Compare ideas with a partner, then record a conclusion.')
        body = textbox(2, title, (650000, 500000, 11000000, 850000), 3000, True)
        body += textbox(3, text, (650000, 2100000, 5200000, 3200000))
        links = [('rLayout', R + '/slideLayout', '../slideLayouts/slideLayout1.xml'),
                 ('rNotes', R + '/notesSlide', f'../notesSlides/notesSlide{i}.xml')]
        image_number = (i + 1) // 2 if i % 2 and (i + 1) // 2 <= image_count else None
        if workload != 'classroom':
            body += rectangle(4, (650000, 1550000, 10500000, 70000), '197568')
            body += rectangle(5, (650000, 6050000, 3500000, 100000), 'E9B45B')
        if workload == 'standard' and i == 49:
            body += table()
        elif workload == 'standard' and i == 50:
            body += frame(6, 'Generated editable chart', C, f'<c:chart xmlns:c="{C}" r:id="rChart"/>')
            links.append(('rChart', R + '/chart', '../charts/chart1.xml'))
            add('ppt/charts/chart1.xml', chart(), CT + 'drawingml.chart+xml')
        elif image_number is not None:
            body += picture(6, (6300000, 2050000, 5200000, 2925000))
            links.append(('rImage', R + '/image', f'../media/image{image_number}.png'))
        elif workload != 'classroom':
            body += rectangle(6, (6300000, 2050000, 5200000, 2925000), 'E8EFED')
        presentation_part(f'ppt/slides/slide{i}.xml', XML + f'<p:sld {NS}>'
                          '<p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/>'
                          '</a:solidFill><a:effectLst/></p:bgPr></p:bg>'
                          f'<p:spTree>{GROUP}{body}</p:spTree></p:cSld>'
                          '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>', 'slide')
        add(f'ppt/slides/_rels/slide{i}.xml.rels', rels(links))
        notes = textbox(2, f'Presenter-only note for {workload} slide {i:03d}.\n'
                        'Allow one minute for observation. Invite two explanations.\n'
                        'Fixture content is invented; no learner data is recorded.',
                        (650000, 1000000, 5500000, 5000000), notes=True)
        presentation_part(f'ppt/notesSlides/notesSlide{i}.xml', XML + f'<p:notes {NS}>'
                          f'<p:cSld><p:spTree>{GROUP}{notes}</p:spTree></p:cSld>'
                          '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>', 'notesSlide')
        add(f'ppt/notesSlides/_rels/notesSlide{i}.xml.rels', rels([
            ('rSlide', R + '/slide', f'../slides/slide{i}.xml'),
            ('rMaster', R + '/notesMaster', '../notesMasters/notesMaster1.xml')]))
    add('[Content_Types].xml', XML + '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Default Extension="png" ContentType="image/png"/>'
        + ''.join(f'<Override PartName="/{name}" ContentType="{kind}"/>' for name, kind in overrides)
        + '</Types>')
    return parts


def zip_info(name):
    info = ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
    info.compress_type = ZIP_DEFLATED
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    return info


def write_package(path, parts, oversized=False):
    with path.open('xb') as output, ZipFile(output, 'w', compression=ZIP_DEFLATED, compresslevel=9) as package:
        for name, contents in sorted(parts.items()):
            package.writestr(zip_info(name), contents)
        if oversized:
            # Stream a bounded 50 MiB + 1 byte entry, just above open_and_validate's
            # per-entry ceiling. No giant Python allocation or forged ZIP headers.
            with package.open(zip_info('fixture-oversized.bin'), 'w') as member:
                for _ in range(50):
                    member.write(b'0' * (1024 * 1024))
                member.write(b'0')


def validate(path, workload):
    """Structural checks are not a substitute for opening and saving in an app."""
    count, image_count, width, height, limit = PROFILES[workload]
    namespaces = {'p': P, 'a': A, 'c': C}
    with ZipFile(path) as package:
        if package.testzip() is not None:
            raise ValueError('ZIP integrity check failed')
        names = set(package.namelist())
        images = [name for name in names if name.startswith('ppt/media/')]
        if len(images) != image_count:
            raise ValueError('Image count mismatch')
        for name in images:
            image = package.read(name)
            if image[:8] != b'\x89PNG\r\n\x1a\n' or struct.unpack('>II', image[16:24]) != (width, height):
                raise ValueError(f'PNG dimensions mismatch: {name}')
        for name in names:
            if name.endswith(('.xml', '.rels')):
                root = ET.fromstring(package.read(name))
                if name.endswith('.rels'):
                    parent = '' if name == '_rels/.rels' else posixpath.dirname(posixpath.dirname(name))
                    for relation in root:
                        target = posixpath.normpath(posixpath.join(parent, relation.attrib['Target']))
                        if relation.attrib.get('TargetMode') == 'External' or target not in names:
                            raise ValueError(f'External or missing relationship: {name} -> {target}')
        for i in range(1, count + 1):
            root = ET.fromstring(package.read(f'ppt/slides/slide{i}.xml'))
            tree = root.find('p:cSld/p:spTree', namespaces)
            shapes = [node for node in tree if node.tag not in (f'{{{P}}}nvGrpSpPr', f'{{{P}}}grpSpPr')]
            textboxes = root.findall('.//p:sp/p:txBody', namespaces)
            if len(textboxes) != 2 or (workload != 'classroom' and len(shapes) != 5):
                raise ValueError(f'Shape count mismatch: slide {i}')
            note = ET.fromstring(package.read(f'ppt/notesSlides/notesSlide{i}.xml'))
            if not note.findall('.//a:t', namespaces):
                raise ValueError(f'Missing notes: slide {i}')
        if workload == 'standard':
            root = ET.fromstring(package.read('ppt/slides/slide49.xml'))
            rows = root.findall('.//a:tbl/a:tr', namespaces)
            if len(rows) != 10 or any(len(row.findall('a:tc', namespaces)) != 5 for row in rows):
                raise ValueError('Table dimensions mismatch')
            ET.fromstring(package.read('ppt/charts/chart1.xml'))
    if limit is not None and path.stat().st_size > limit:
        raise ValueError('Fixture exceeds the compressed file budget')


def record(path, **metadata):
    digest = hashlib.sha256()
    with path.open('rb') as data:
        for chunk in iter(lambda: data.read(1024 * 1024), b''):
            digest.update(chunk)
    return {'file': path.name, 'bytes': path.stat().st_size, 'sha256': digest.hexdigest(), **metadata}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', required=True, type=Path)
    parser.add_argument('--workload', choices=[*PROFILES, 'all'], default='classroom')
    parser.add_argument('--include-negative', action='store_true')
    args = parser.parse_args()
    workloads = list(PROFILES) if args.workload == 'all' else [args.workload]
    expected = [f'{name}.pptx' for name in workloads] + ['fixture-manifest.json', 'LICENSE.txt']
    if args.include_negative:
        expected += ['corrupt-missing-presentation.pptx', 'oversized-entry.pptx', 'unsupported-group.pptx']
    if any((args.output_dir / name).exists() for name in expected):
        parser.error('Output exists; choose a fresh directory. No files were replaced.')
    license_text = (Path(__file__).resolve().parent.parent / 'LICENSE').read_text(encoding='utf-8')
    args.output_dir.mkdir(parents=True, exist_ok=True)
    results = []
    for workload in workloads:
        parts = build_parts(workload)
        path = args.output_dir / f'{workload}.pptx'
        write_package(path, parts)
        validate(path, workload)
        slides, images, width, height, _ = PROFILES[workload]
        results.append(record(path, workload=workload, slides=slides, images=images,
                              image_format='PNG', image_dimensions=[width, height],
                              textboxes_per_slide=2, notes_slides=slides,
                              objects_per_slide='2 or 3' if workload == 'classroom' else 5,
                              table_dimensions=[10, 5] if workload == 'standard' else None,
                              charts=1 if workload == 'standard' else 0,
                              expected='Open, edit, save, reopen; structural checks passed'))
    if args.include_negative:
        # Reuse the small valid classroom workload, keeping failures separate.
        base = build_parts('classroom')
        corrupt = dict(base)
        del corrupt['ppt/presentation.xml']
        path = args.output_dir / 'corrupt-missing-presentation.pptx'
        write_package(path, corrupt)
        results.append(record(path, expected='Reject missing presentation without replacing the current deck'))
        path = args.output_dir / 'oversized-entry.pptx'
        write_package(path, base, oversized=True)
        results.append(record(path, oversized_uncompressed_entry_bytes=50 * 1024 * 1024 + 1,
                              expected='Reject EntryTooLarge before decompressing oversized entry'))
        unsupported = dict(base)
        group = ('<p:grpSp>' + GROUP.replace('id="1"', 'id="20"')
                 + rectangle(21, (0, 0, 1000000, 1000000), '197568') + '</p:grpSp>')
        unsupported['ppt/slides/slide1.xml'] = unsupported['ppt/slides/slide1.xml'].replace(
            b'</p:spTree>', group.encode('utf-8') + b'</p:spTree>')
        path = args.output_dir / 'unsupported-group.pptx'
        write_package(path, unsupported)
        results.append(record(path, expected='Open with a slide 1 unsupported-group warning; preserve group on save'))
    manifest = {
        'schema': 1, 'generator': 'scripts/generate-perf-fixtures.py',
        'license': 'Apache-2.0', 'license_file': 'LICENSE.txt',
        'python_version': sys.version.split()[0], 'zlib_version': zlib.ZLIB_RUNTIME_VERSION,
        'fixture_spec': 'docs/LOW_RESOURCE_REQUIREMENTS.md',
        'limitations': [
            'Synthetic tiled PNG images replace specified JPEG photographs; compressed bytes are not photo representative.',
            'No font files are embedded; Liberation Sans must be installed or substitution must be recorded.',
            'ZIP/XML checks are structural only; native open/save, external-office interoperability and hardware performance are unverified.',
            'Archive order, timestamps, pixels and content are fixed; repeatability assumes the same Python/zlib toolchain.',
            'Stress uses 1280x720 images; the requirements do not yet specify stress-image dimensions.',
        ],
        'files': results,
    }
    text = json.dumps(manifest, indent=2, sort_keys=True) + '\n'
    with (args.output_dir / 'LICENSE.txt').open('x', encoding='utf-8') as output:
        output.write(license_text)
    with (args.output_dir / 'fixture-manifest.json').open('x', encoding='utf-8') as output:
        output.write(text)
    print(text, end='')


if __name__ == '__main__':
    main()
