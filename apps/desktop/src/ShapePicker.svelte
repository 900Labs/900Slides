<script lang="ts">
  /** A geometric shape offered by the picker, paired with its add_shape kind. */
  interface ShapeOption {
    kind: string
    label: string
  }

  /** Props for the shape picker flyout. */
  interface Props {
    /** Called with the chosen geometry kind when a shape is clicked. */
    onPick: (kind: string) => void
  }

  let { onPick }: Props = $props()

  /** Geometry kinds supported by the backend `add_shape` command. */
  const SHAPES: ShapeOption[] = [
    { kind: 'rectangle', label: 'Rectangle' },
    { kind: 'rounded_rectangle', label: 'Rounded' },
    { kind: 'ellipse', label: 'Ellipse' },
    { kind: 'triangle', label: 'Triangle' },
    { kind: 'line', label: 'Line' },
    { kind: 'arrow', label: 'Arrow' },
    { kind: 'right_arrow_callout', label: 'Callout' },
    { kind: 'star5', label: 'Star' },
  ]
</script>

<div class="shape-picker" role="dialog" aria-label="Choose a shape">
  <div class="shape-picker-grid">
    {#each SHAPES as shape (shape.kind)}
      <button
        class="shape-cell"
        type="button"
        title={shape.label}
        aria-label={shape.label}
        onclick={() => onPick(shape.kind)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          {#if shape.kind === 'rectangle'}
            <rect x="3" y="6" width="18" height="12" rx="1"></rect>
          {:else if shape.kind === 'rounded_rectangle'}
            <rect x="3" y="6" width="18" height="12" rx="5"></rect>
          {:else if shape.kind === 'ellipse'}
            <ellipse cx="12" cy="12" rx="9" ry="6"></ellipse>
          {:else if shape.kind === 'triangle'}
            <path d="M12 4 21 20 3 20 Z"></path>
          {:else if shape.kind === 'line'}
            <path d="M4 18 L20 6"></path>
          {:else if shape.kind === 'arrow'}
            <path d="M3 9 H12 V5 L21 12 L12 19 V15 H3 Z"></path>
          {:else if shape.kind === 'right_arrow_callout'}
            <path d="M3 6 H11 V3 L20 12 L11 21 V18 H3 Z"></path>
          {:else if shape.kind === 'star5'}
            <path
              d="M12 3 L14.5 9 L21 9.3 L16 13.5 L17.8 20 L12 16.3 L6.2 20 L8 13.5 L3 9.3 L9.5 9 Z"
            ></path>
          {/if}
        </svg>
        <span class="shape-cell-label">{shape.label}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .shape-picker {
    background: #fff;
    border: 1px solid #ccc;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    padding: 0.4rem;
  }
  .shape-picker-grid {
    display: grid;
    grid-template-columns: repeat(4, 56px);
    gap: 2px;
  }
  .shape-cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 0.3rem 0.15rem;
    background: #fafafa;
    border: 1px solid transparent;
    border-radius: 3px;
    cursor: pointer;
  }
  .shape-cell:hover {
    background: #e8f1fb;
    border-color: #0070c0;
  }
  .shape-cell svg {
    width: 22px;
    height: 22px;
    fill: #334;
    stroke: #334;
    stroke-width: 1.6;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  /* Line geometry is an open path; the rest read better as filled outlines. */
  .shape-cell svg path[d^='M4 18'] {
    fill: none;
  }
  .shape-cell-label {
    font-size: 0.62rem;
    color: #555;
    line-height: 1;
  }
</style>
