<script lang="ts">
  import { untrack } from 'svelte'
  import type {
    CategorySeriesDto,
    ChartDataDto,
    ChartShapeSnapshot,
    ChartTypeDto,
    XYPointDto,
    XYSeriesDto,
  } from './lib/types'

  /** Props for the chart data-table editor overlay. */
  interface Props {
    /** Chart shape being edited. */
    chart: ChartShapeSnapshot
    /** Stable slide identifier. */
    slideId: string
    /** Index of the chart shape on the slide. */
    shapeIndex: number
    /** Called when the user applies changes. */
    onApply: (detail: {
      slideId: string
      shapeIndex: number
      chartType: ChartTypeDto
      data: ChartDataDto
      title: string
    }) => Promise<void> | void
    /** Called when the editor is dismissed. */
    onClose: () => void
  }

  let { chart, slideId, shapeIndex, onApply, onClose }: Props = $props()

  /** Editable local copy of the chart type. */
  let chartType = $state<ChartTypeDto>(untrack(() => chart.chartType))
  /** Editable local copy of the chart title. */
  let title = $state(untrack(() => chart.title ?? ''))
  /** Editable local copy of the chart data. */
  let data = $state<ChartDataDto>(untrack(() => structuredClone(chart.data)))

  const CHART_TYPES: ChartTypeDto[] = ['bar', 'column', 'line', 'area', 'pie', 'scatter']

  /** Returns true when the current chart type uses category data. */
  function isCategory(type: ChartTypeDto): boolean {
    return type !== 'scatter'
  }

  /** Converts category data to XY data. */
  function categoryToXY(categoryData: ChartDataDto): ChartDataDto {
    if (categoryData.kind !== 'category') return categoryData
    const series: XYSeriesDto[] = categoryData.value.series.map((s) => ({
      name: s.name,
      points: s.values.map((value, index) => ({ x: index, y: value })),
    }))
    return { kind: 'xy', value: { series } }
  }

  /** Converts XY data to category data using numeric categories. */
  function xyToCategory(xyData: ChartDataDto): ChartDataDto {
    if (xyData.kind !== 'xy') return xyData
    const maxPoints = Math.max(1, ...xyData.value.series.map((s) => s.points.length))
    const categories = Array.from({ length: maxPoints }, (_, i) => `Item ${i + 1}`)
    const series: CategorySeriesDto[] = xyData.value.series.map((s) => ({
      name: s.name,
      values: categories.map((_, i) => s.points[i]?.y ?? 0),
    }))
    return { kind: 'category', value: { categories, series } }
  }

  /** Handles chart type changes, converting data between kinds when necessary. */
  function handleTypeChange(event: Event): void {
    const select = event.target as HTMLSelectElement
    const newType = select.value as ChartTypeDto
    if (isCategory(chartType) && !isCategory(newType)) {
      data = categoryToXY(data)
    } else if (!isCategory(chartType) && isCategory(newType)) {
      data = xyToCategory(data)
    }
    chartType = newType
  }

  /** Ensures category data always has at least one series and one category. */
  function ensureCategory(): void {
    if (data.kind !== 'category') return
    if (data.value.categories.length === 0) {
      data = {
        kind: 'category',
        value: { categories: ['Category 1'], series: [{ name: 'Series 1', values: [0] }] },
      }
    } else if (data.value.series.length === 0) {
      data = {
        kind: 'category',
        value: { ...data.value, series: [{ name: 'Series 1', values: data.value.categories.map(() => 0) }] },
      }
    }
  }

  /** Ensures XY data always has at least one series with one point. */
  function ensureXY(): void {
    if (data.kind !== 'xy') return
    if (data.value.series.length === 0) {
      data = { kind: 'xy', value: { series: [{ name: 'Series 1', points: [{ x: 0, y: 0 }] }] } }
    }
    data.value.series = data.value.series.map((s) => ({
      ...s,
      points: s.points.length > 0 ? s.points : [{ x: 0, y: 0 }],
    }))
  }

  /** Adds a new category column to category data. */
  function addCategory(): void {
    if (data.kind !== 'category') return
    const nextIndex = data.value.categories.length + 1
    data = {
      kind: 'category',
      value: {
        categories: [...data.value.categories, `Category ${nextIndex}`],
        series: data.value.series.map((s) => ({ ...s, values: [...s.values, 0] })),
      },
    }
  }

  /** Removes a category column from category data. */
  function removeCategory(index: number): void {
    if (data.kind !== 'category') return
    if (data.value.categories.length <= 1) return
    data = {
      kind: 'category',
      value: {
        categories: data.value.categories.filter((_, i) => i !== index),
        series: data.value.series.map((s) => ({
          ...s,
          values: s.values.filter((_, i) => i !== index),
        })),
      },
    }
  }

  /** Adds a new series to category data. */
  function addCategorySeries(): void {
    if (data.kind !== 'category') return
    const nextIndex = data.value.series.length + 1
    data = {
      kind: 'category',
      value: {
        ...data.value,
        series: [
          ...data.value.series,
          { name: `Series ${nextIndex}`, values: data.value.categories.map(() => 0) },
        ],
      },
    }
  }

  /** Removes a series from category data. */
  function removeCategorySeries(index: number): void {
    if (data.kind !== 'category') return
    if (data.value.series.length <= 1) return
    data = {
      kind: 'category',
      value: {
        ...data.value,
        series: data.value.series.filter((_, i) => i !== index),
      },
    }
  }

  /** Updates a category name. */
  function updateCategory(index: number, value: string): void {
    if (data.kind !== 'category') return
    data = {
      kind: 'category',
      value: {
        ...data.value,
        categories: data.value.categories.map((c, i) => (i === index ? value : c)),
      },
    }
  }

  /** Updates a series name for category data. */
  function updateCategorySeriesName(index: number, value: string): void {
    if (data.kind !== 'category') return
    data = {
      kind: 'category',
      value: {
        ...data.value,
        series: data.value.series.map((s, i) => (i === index ? { ...s, name: value } : s)),
      },
    }
  }

  /** Updates a single value in category data. */
  function updateCategoryValue(seriesIndex: number, valueIndex: number, value: string): void {
    if (data.kind !== 'category') return
    const parsed = Number.parseFloat(value)
    if (Number.isNaN(parsed)) return
    data = {
      kind: 'category',
      value: {
        ...data.value,
        series: data.value.series.map((s, si) =>
          si === seriesIndex
            ? { ...s, values: s.values.map((v, vi) => (vi === valueIndex ? parsed : v)) }
            : s,
        ),
      },
    }
  }

  /** Adds a new series to XY data. */
  function addXYSeries(): void {
    if (data.kind !== 'xy') return
    const nextIndex = data.value.series.length + 1
    data = {
      kind: 'xy',
      value: {
        series: [...data.value.series, { name: `Series ${nextIndex}`, points: [{ x: 0, y: 0 }] }],
      },
    }
  }

  /** Removes a series from XY data. */
  function removeXYSeries(index: number): void {
    if (data.kind !== 'xy') return
    if (data.value.series.length <= 1) return
    data = {
      kind: 'xy',
      value: {
        series: data.value.series.filter((_, i) => i !== index),
      },
    }
  }

  /** Adds a point to an XY series. */
  function addXYPoint(seriesIndex: number): void {
    if (data.kind !== 'xy') return
    data = {
      kind: 'xy',
      value: {
        series: data.value.series.map((s, i) =>
          i === seriesIndex ? { ...s, points: [...s.points, { x: 0, y: 0 }] } : s,
        ),
      },
    }
  }

  /** Removes a point from an XY series. */
  function removeXYPoint(seriesIndex: number, pointIndex: number): void {
    if (data.kind !== 'xy') return
    const series = data.value.series[seriesIndex]
    if (!series || series.points.length <= 1) return
    data = {
      kind: 'xy',
      value: {
        series: data.value.series.map((s, i) =>
          i === seriesIndex ? { ...s, points: s.points.filter((_, pi) => pi !== pointIndex) } : s,
        ),
      },
    }
  }

  /** Updates an XY series name. */
  function updateXYSeriesName(index: number, value: string): void {
    if (data.kind !== 'xy') return
    data = {
      kind: 'xy',
      value: {
        series: data.value.series.map((s, i) => (i === index ? { ...s, name: value } : s)),
      },
    }
  }

  /** Updates a single (x, y) point coordinate. */
  function updateXYPoint(
    seriesIndex: number,
    pointIndex: number,
    field: keyof XYPointDto,
    value: string,
  ): void {
    if (data.kind !== 'xy') return
    const parsed = Number.parseFloat(value)
    if (Number.isNaN(parsed)) return
    data = {
      kind: 'xy',
      value: {
        series: data.value.series.map((s, si) =>
          si === seriesIndex
            ? {
                ...s,
                points: s.points.map((p, pi) => (pi === pointIndex ? { ...p, [field]: parsed } : p)),
              }
            : s,
        ),
      },
    }
  }

  /** Applies the current editor state and closes the overlay. */
  async function handleApply(): Promise<void> {
    if (isCategory(chartType)) {
      ensureCategory()
    } else {
      ensureXY()
    }
    await onApply({ slideId, shapeIndex, chartType, data, title })
    onClose()
  }

  /** Cancels editing and closes the overlay. */
  function handleCancel(): void {
    onClose()
  }
</script>

<div class="overlay" role="dialog" aria-label="Chart data editor">
  <div class="panel">
    <div class="header">
      <label class="field">
        Title
        <input type="text" bind:value={title} placeholder="Chart title" />
      </label>
      <label class="field">
        Type
        <select value={chartType} onchange={handleTypeChange}>
          {#each CHART_TYPES as type}
            <option value={type}>{type.charAt(0).toUpperCase() + type.slice(1)}</option>
          {/each}
        </select>
      </label>
      <div class="actions">
        <button onclick={handleApply} type="button">Apply</button>
        <button onclick={handleCancel} type="button">Cancel</button>
      </div>
    </div>

    <div class="body">
      {#if data.kind === 'category'}
        {@const categories = data.value.categories}
        {@const series = data.value.series}
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                <th class="corner"></th>
                {#each categories as _, index}
                  <th>
                    <input
                      type="text"
                      value={categories[index]}
                      onchange={(event) => updateCategory(index, (event.target as HTMLInputElement).value)}
                      aria-label={`Category ${index + 1}`}
                    />
                    <button
                      class="small"
                      onclick={() => removeCategory(index)}
                      disabled={categories.length <= 1}
                      type="button"
                      title="Remove category"
                    >−</button>
                  </th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each series as s, seriesIndex}
                <tr>
                  <th>
                    <input
                      type="text"
                      value={s.name}
                      onchange={(event) =>
                        updateCategorySeriesName(seriesIndex, (event.target as HTMLInputElement).value)}
                      aria-label={`Series ${seriesIndex + 1} name`}
                    />
                  </th>
                  {#each categories as _, valueIndex}
                    <td>
                      <input
                        type="number"
                        step="any"
                        value={s.values[valueIndex] ?? 0}
                        onchange={(event) =>
                          updateCategoryValue(
                            seriesIndex,
                            valueIndex,
                            (event.target as HTMLInputElement).value,
                          )}
                        aria-label={`Value ${seriesIndex + 1}, ${valueIndex + 1}`}
                      />
                    </td>
                  {/each}
                  <td>
                    <button
                      class="small"
                      onclick={() => removeCategorySeries(seriesIndex)}
                      disabled={series.length <= 1}
                      type="button"
                      title="Remove series"
                    >−</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <div class="row-actions">
          <button onclick={addCategorySeries} type="button">+ Series</button>
          <button onclick={addCategory} type="button">+ Category</button>
        </div>
      {:else}
        <div class="xy-editor">
          {#each data.value.series as s, seriesIndex}
            <div class="xy-series">
              <div class="series-header">
                <input
                  type="text"
                  value={s.name}
                  onchange={(event) =>
                    updateXYSeriesName(seriesIndex, (event.target as HTMLInputElement).value)}
                  aria-label={`Series ${seriesIndex + 1} name`}
                />
                <button
                  class="small"
                  onclick={() => removeXYSeries(seriesIndex)}
                  disabled={data.value.series.length <= 1}
                  type="button"
                  title="Remove series"
                >− Series</button>
              </div>
              <table class="data-table">
                <thead>
                  <tr>
                    <th>X</th>
                    <th>Y</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {#each s.points as point, pointIndex}
                    <tr>
                      <td>
                        <input
                          type="number"
                          step="any"
                          value={point.x}
                          onchange={(event) =>
                            updateXYPoint(
                              seriesIndex,
                              pointIndex,
                              'x',
                              (event.target as HTMLInputElement).value,
                            )}
                          aria-label={`Point ${pointIndex + 1} x`}
                        />
                      </td>
                      <td>
                        <input
                          type="number"
                          step="any"
                          value={point.y}
                          onchange={(event) =>
                            updateXYPoint(
                              seriesIndex,
                              pointIndex,
                              'y',
                              (event.target as HTMLInputElement).value,
                            )}
                          aria-label={`Point ${pointIndex + 1} y`}
                        />
                      </td>
                      <td>
                        <button
                          class="small"
                          onclick={() => removeXYPoint(seriesIndex, pointIndex)}
                          disabled={s.points.length <= 1}
                          type="button"
                          title="Remove point"
                        >−</button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
              <button onclick={() => addXYPoint(seriesIndex)} type="button">+ Point</button>
            </div>
          {/each}
          <button onclick={addXYSeries} type="button">+ Series</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.4);
  }
  .panel {
    width: min(90vw, 800px);
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    background: #fff;
    border: 1px solid #999;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.25);
    border-radius: 4px;
    overflow: hidden;
  }
  .header {
    display: flex;
    gap: 1rem;
    align-items: flex-end;
    padding: 1rem;
    border-bottom: 1px solid #ddd;
    background: #f8f8f8;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85rem;
    color: #555;
  }
  .field input,
  .field select {
    padding: 0.3rem 0.4rem;
    font-size: 0.9rem;
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 0.5rem;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 1rem;
  }
  .table-wrap {
    overflow: auto;
    max-height: 50vh;
  }
  .data-table {
    border-collapse: collapse;
    font-size: 0.85rem;
  }
  .data-table th,
  .data-table td {
    border: 1px solid #ddd;
    padding: 0.2rem;
    min-width: 80px;
  }
  .data-table th {
    background: #f4f4f4;
  }
  .data-table th.corner {
    min-width: 120px;
  }
  .data-table input {
    width: 100%;
    border: none;
    padding: 0.2rem;
    font-size: 0.85rem;
    background: transparent;
  }
  .data-table input[type='number'] {
    text-align: right;
  }
  .data-table input:focus {
    outline: 1px solid #0070c0;
  }
  .row-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }
  .xy-editor {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .xy-series {
    border: 1px solid #ddd;
    border-radius: 4px;
    padding: 0.75rem;
    background: #fafafa;
  }
  .series-header {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  .series-header input {
    flex: 1;
    padding: 0.25rem;
  }
  button.small {
    padding: 0.1rem 0.4rem;
    font-size: 0.8rem;
  }
</style>
