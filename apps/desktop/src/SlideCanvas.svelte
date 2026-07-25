<script lang="ts">
  import type {
    ColorDto,
    CropDto,
    DashStyleDto,
    FillDto,
    GeometryDto,
    GeometricShapeSnapshot,
    ImageShapeSnapshot,
    MediaMap,
    ParagraphDto,
    PassthroughSnapshot,
    SlideSnapshot,
    StyleDto,
    TextBoxSnapshot,
  } from './lib/types'

  /** Props for the slide canvas. */
  interface Props {
    /** Slide to render. */
    slide: SlideSnapshot
    /** Deck background color. */
    background: ColorDto
    /** Media store, base64-encoded, used to render image shapes. */
    media?: MediaMap
    /** Callback invoked when a text box is edited. */
    onEditTextBox?: (detail: {
      slideId: string
      shapeIndex: number
      paragraphs: ParagraphDto[]
    }) => void
    /** Whether the canvas is read-only (presenter mode). */
    readonly?: boolean
  }

  let { slide, background, media, onEditTextBox, readonly = false }: Props = $props()

  /** EMU to CSS pixels for a 1280x720 (16:9) canvas. */
  const EMU_TO_PX = 1.0 / 9525.0

  /** Converts EMU to a pixel CSS string. */
  function toPx(emu: number): string {
    return `${emu * EMU_TO_PX}px`
  }

  /** Converts a ColorDto to a CSS rgba string. */
  function toRgba(color: ColorDto): string {
    return `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`
  }

  /** Formats a ColorDto as an opaque `#rrggbb` hex string. */
  function toHex(color: ColorDto): string {
    const h = (n: number) => n.toString(16).padStart(2, '0')
    return `#${h(color.r)}${h(color.g)}${h(color.b)}`
  }

  /** Builds a `data:` URI for a media entry. */
  function dataUri(entry: { mime: string; bytes: string }): string {
    return `data:${entry.mime};base64,${entry.bytes}`
  }

  /** Joins all runs in all paragraphs into a single editable string. */
  function textFromParagraphs(paragraphs: ParagraphDto[]): string {
    return paragraphs
      .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
      .join('\n')
  }

  /** Concatenates the run text of a single paragraph. */
  function paragraphText(paragraph: ParagraphDto): string {
    return paragraph.runs.map((run) => run.text).join('')
  }

  /** Builds a paragraph DTO, preserving original runs when the text is unchanged. */
  function buildParagraph(
    text: string,
    original: ParagraphDto | undefined,
  ): ParagraphDto {
    if (original && paragraphText(original) === text) {
      return original
    }
    return {
      runs: text ? [{ text, bold: false, italic: false, underline: false }] : [],
      listStyle: original?.listStyle ?? 'none',
    }
  }

  /** Emits a text-box edit command after splitting the textarea value into paragraphs. */
  function handleBlur(event: FocusEvent, shapeIndex: number): void {
    if (!onEditTextBox) return
    const target = event.target as HTMLTextAreaElement
    const textBox = (slide.shapes[shapeIndex].value as TextBoxSnapshot)
    const originalParagraphs = textBox.paragraphs
    const lines = target.value.split('\n')

    const newParagraphs: ParagraphDto[] = lines.map((line, index) =>
      buildParagraph(line, originalParagraphs[index]),
    )

    const changed =
      newParagraphs.length !== originalParagraphs.length ||
      newParagraphs.some((paragraph, index) => {
        const original = originalParagraphs[index]
        if (!original) return true
        if (paragraph.runs.length !== original.runs.length) return true
        return paragraph.runs.some(
          (run, runIndex) =>
            run.text !== original.runs[runIndex]?.text ||
            run.bold !== original.runs[runIndex]?.bold ||
            run.italic !== original.runs[runIndex]?.italic ||
            run.underline !== original.runs[runIndex]?.underline,
        )
      })

    if (changed) {
      onEditTextBox({
        slideId: slide.id,
        shapeIndex,
        paragraphs: newParagraphs,
      })
    }
  }

  /** Dash pattern in EMU for a given dash style, matching slides-render. */
  function dashArray(dash: DashStyleDto): string | undefined {
    switch (dash) {
      case 'dash':
        return '300000,150000'
      case 'dot':
        return '60000,60000'
      case 'dash_dot':
        return '300000,150000,60000,150000'
      default:
        return undefined
    }
  }

  /** Builds the `fill`, `stroke`, and `stroke-dasharray` attributes for a style. */
  function styleAttributes(style: StyleDto): string {
    let attrs = ''
    const fill = style.fill as FillDto | undefined
    if (fill && fill.solid) {
      attrs += ` fill="${toHex(fill.solid)}"`
    } else {
      attrs += ' fill="none"'
    }
    if (style.outline) {
      attrs += ` stroke="${toHex(style.outline.color)}" stroke-width="${style.outline.widthEmu}"`
      const dash = dashArray(style.outline.dash)
      if (dash) attrs += ` stroke-dasharray="${dash}"`
    }
    return attrs
  }

  /** Returns the transform attribute for a non-zero rotation around the frame center. */
  function rotateAttr(rotation: number, frameWidth: number, frameHeight: number): string {
    if (!rotation) return ''
    return ` transform="rotate(${rotation},${frameWidth / 2},${frameHeight / 2})"`
  }

  /** Builds the SVG path for a block arrow filling the frame, matching slides-render. */
  function arrowPath(w: number, h: number): string {
    const shaftTop = h / 3
    const shaftBot = (2 * h) / 3
    const headBase = (2 * w) / 3
    const tip = w
    const mid = h / 2
    return `M 0,${shaftTop} L ${headBase},${shaftTop} L ${headBase},0 L ${tip},${mid} L ${headBase},${h} L ${headBase},${shaftBot} L 0,${shaftBot} Z`
  }

  /** Builds the SVG path for a right-arrow callout, matching slides-render. */
  function rightArrowCalloutPath(w: number, h: number): string {
    const bodyRight = (2 * w) / 3
    const tip = w
    const mid = h / 2
    const q1 = h / 4
    const q3 = (3 * h) / 4
    return `M 0,0 L ${bodyRight},0 L ${bodyRight},${q1} L ${tip},${mid} L ${bodyRight},${q3} L ${bodyRight},${h} L 0,${h} Z`
  }

  /** Builds the SVG path for a five-pointed star, matching slides-render. */
  function star5Path(w: number, h: number): string {
    const cx = w / 2
    const cy = h / 2
    const outer = Math.min(w, h) / 2
    const inner = outer * 0.3819660112501051
    let d = 'M'
    for (let i = 0; i < 10; i += 1) {
      const angle = ((-90 + i * 36) * Math.PI) / 180
      const radius = i % 2 === 0 ? outer : inner
      const px = cx + radius * Math.cos(angle)
      const py = cy + radius * Math.sin(angle)
      d += ` ${px},${py}`
    }
    return `${d} Z`
  }

  /** Builds the inner SVG element for a geometry, sized to its frame in EMU. */
  function geometryMarkup(
    geometry: GeometryDto,
    style: StyleDto,
    frameWidth: number,
    frameHeight: number,
    rotation: number,
  ): string {
    const attrs = styleAttributes(style)
    const rotate = rotateAttr(rotation, frameWidth, frameHeight)
    let shape = ''
    if (geometry === 'rectangle') {
      shape = `<rect x="0" y="0" width="${frameWidth}" height="${frameHeight}"${attrs}${rotate}/>`
    } else if (geometry === 'ellipse') {
      shape = `<ellipse cx="${frameWidth / 2}" cy="${frameHeight / 2}" rx="${frameWidth / 2}" ry="${frameHeight / 2}"${attrs}${rotate}/>`
    } else if (geometry === 'triangle') {
      shape = `<polygon points="0,${frameHeight} ${frameWidth},${frameHeight} ${frameWidth / 2},0"${attrs}${rotate}/>`
    } else if (geometry === 'line') {
      shape = `<line x1="0" y1="${frameHeight / 2}" x2="${frameWidth}" y2="${frameHeight / 2}"${attrs}${rotate}/>`
    } else if (geometry === 'arrow') {
      shape = `<path d="${arrowPath(frameWidth, frameHeight)}"${attrs}${rotate}/>`
    } else if (geometry === 'right_arrow_callout') {
      shape = `<path d="${rightArrowCalloutPath(frameWidth, frameHeight)}"${attrs}${rotate}/>`
    } else if (geometry === 'star5') {
      shape = `<path d="${star5Path(frameWidth, frameHeight)}"${attrs}${rotate}/>`
    } else if (typeof geometry === 'object' && 'rounded_rectangle' in geometry) {
      const radius = geometry.rounded_rectangle.radius
      shape = `<rect x="0" y="0" width="${frameWidth}" height="${frameHeight}" rx="${radius}" ry="${radius}"${attrs}${rotate}/>`
    }
    return shape
  }

  /** Builds a complete `<svg>` document for a geometric shape. */
  function geometricSvg(shape: GeometricShapeSnapshot): string {
    const { frame } = shape.transform
    const inner = geometryMarkup(
      shape.geometry,
      shape.style,
      frame.width,
      frame.height,
      shape.transform.rotation,
    )
    let filterOpen = ''
    let filterClose = ''
    if (shape.style.shadow) {
      const s = shape.style.shadow
      filterOpen = `<defs><filter id="sh" x="-50%" y="-50%" width="200%" height="200%"><feDropShadow dx="${s.offsetX}" dy="${s.offsetY}" stdDeviation="${s.blur}" flood-color="${toHex(s.color)}" flood-opacity="${s.opacity}"/></filter></defs>`
      filterOpen += `<g filter="url(#sh)">`
      filterClose = `</g>`
    }
    return `<svg viewBox="0 0 ${frame.width} ${frame.height}" xmlns="http://www.w3.org/2000/svg" preserveAspectRatio="none">${filterOpen}${inner}${filterClose}</svg>`
  }

  /** Inner image style: fills the frame, or reveals only the cropped region. */
  function imageInnerStyle(crop: CropDto | undefined): string {
    if (!crop) {
      return 'width:100%;height:100%;object-fit:fill;'
    }
    const visW = Math.max(1e-6, 1 - crop.left - crop.right)
    const visH = Math.max(1e-6, 1 - crop.top - crop.bottom)
    const scaledW = 100 / visW
    const scaledH = 100 / visH
    const offsetX = crop.left * scaledW
    const offsetY = crop.top * scaledH
    return `position:absolute;width:${scaledW}%;height:${scaledH}%;left:-${offsetX}%;top:-${offsetY}%;object-fit:fill;`
  }
</script>

<div
  class="canvas"
  style:background-color={toRgba(background)}
  role="application"
  aria-label="Slide canvas"
>
  {#each slide.shapes as shape, shapeIndex}
    {#if shape.kind === 'text_box'}
      {@const textBox = shape.value as TextBoxSnapshot}
      <div
        class="text-box-container"
        style:left={toPx(textBox.frame.x)}
        style:top={toPx(textBox.frame.y)}
        style:width={toPx(textBox.frame.width)}
        style:height={toPx(textBox.frame.height)}
      >
        {#if readonly}
          <div class="text-box-readonly">
            {#each textBox.paragraphs as paragraph}
              <p>
                {#each paragraph.runs as run}
                  <span class:bold={run.bold} class:italic={run.italic} class:underline={run.underline}>{run.text}</span>
                {/each}
              </p>
            {/each}
          </div>
        {:else}
          <textarea
            class="text-box"
            value={textFromParagraphs(textBox.paragraphs)}
            onblur={(event) => handleBlur(event, shapeIndex)}
            aria-label="Editable text box"
          ></textarea>
        {/if}
      </div>
    {:else if shape.kind === 'passthrough'}
      {@const obj = shape.value as PassthroughSnapshot}
      {@const passthroughIndex = slide.shapes.filter((s, i) => s.kind === 'passthrough' && i < shapeIndex).length}
      <div
        class="passthrough"
        style:left={obj.frame ? toPx(obj.frame.x) : undefined}
        style:top={obj.frame ? toPx(obj.frame.y) : `${1 + passthroughIndex * 0.5}rem`}
        style:right={obj.frame ? undefined : '1rem'}
        style:width={obj.frame ? toPx(obj.frame.width) : undefined}
        style:height={obj.frame ? toPx(obj.frame.height) : undefined}
      >
        [preserved object: {obj.label}]
      </div>
    {:else if shape.kind === 'image'}
      {@const image = shape.value as ImageShapeSnapshot}
      {@const entry = media?.[image.mediaRef]}
      {@const frame = image.transform.frame}
      {@const rotation = image.transform.rotation}
      <div
        class="image-container"
        style:left={toPx(frame.x)}
        style:top={toPx(frame.y)}
        style:width={toPx(frame.width)}
        style:height={toPx(frame.height)}
        style:transform={rotation ? `rotate(${rotation}deg)` : undefined}
      >
        {#if entry}
          <img
            class="image"
            style={imageInnerStyle(image.crop)}
            src={dataUri(entry)}
            alt=""
            draggable="false"
          />
        {:else}
          <div class="image-missing">Missing image: {image.mediaRef}</div>
        {/if}
      </div>
    {:else if shape.kind === 'geometric'}
      {@const geometric = shape.value as GeometricShapeSnapshot}
      {@const frame = geometric.transform.frame}
      <div
        class="geometric-container"
        style:left={toPx(frame.x)}
        style:top={toPx(frame.y)}
        style:width={toPx(frame.width)}
        style:height={toPx(frame.height)}
      >
        {@html geometricSvg(geometric)}
      </div>
    {/if}
  {/each}
</div>

<style>
  .canvas {
    position: relative;
    width: 1280px;
    height: 720px;
    flex-shrink: 0;
    box-shadow: 0 0 0 1px #ccc;
    overflow: hidden;
  }
  .text-box-container {
    position: absolute;
  }
  .text-box {
    width: 100%;
    height: 100%;
    border: 1px dashed #999;
    background: transparent;
    resize: none;
    font-family: inherit;
    font-size: 1rem;
    line-height: 1.3;
    padding: 0.25rem;
  }
  .text-box:focus {
    outline: 2px solid #0070c0;
  }
  .text-box-readonly {
    width: 100%;
    height: 100%;
    padding: 0.25rem;
  }
  .text-box-readonly p {
    margin: 0 0 0.5rem;
  }
  .bold {
    font-weight: bold;
  }
  .italic {
    font-style: italic;
  }
  .underline {
    text-decoration: underline;
  }
  .passthrough {
    position: absolute;
    padding: 0.5rem;
    border: 2px dashed #c00;
    color: #c00;
    background: rgba(255, 255, 255, 0.9);
  }
  .image-container {
    position: absolute;
    overflow: hidden;
  }
  .image {
    display: block;
  }
  .image-missing {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #ccc;
    color: #444;
    border: 1px solid #666;
    font-size: 0.8rem;
    text-align: center;
    padding: 0.25rem;
  }
  .geometric-container {
    position: absolute;
  }
  .geometric-container :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
    overflow: visible;
  }
</style>
