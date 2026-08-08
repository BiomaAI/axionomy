import { useEffect, useRef } from "react";
import * as echarts from "echarts/core";
import { ScatterChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { ViewDocument } from "./api";

echarts.use([ScatterChart, GridComponent, TooltipComponent, CanvasRenderer]);

export default function ParetoChart({ document, onSelect }: { document: ViewDocument; onSelect?: (values: string[]) => void }) {
  const container = useRef<HTMLDivElement>(null);
  const front = document.pareto_fronts[0];
  useEffect(() => {
    if (!container.current || !front || front.axes.length < 2) return;
    const chart = echarts.init(container.current);
    const style = getComputedStyle(container.current);
    const selectedColor = style.getPropertyValue("--lime").trim();
    const peerColor = style.getPropertyValue("--blue").trim();
    const backgroundColor = style.getPropertyValue("--bg").trim();
    chart.setOption({
      animationDuration: 450,
      grid: { left: 48, right: 24, top: 30, bottom: 48 },
      xAxis: {
        name: front.axes[0].label,
        nameLocation: "middle",
        nameGap: 31,
        splitLine: { lineStyle: { opacity: 0.12 } },
      },
      yAxis: {
        name: front.axes[1].label,
        nameLocation: "middle",
        nameGap: 34,
        splitLine: { lineStyle: { opacity: 0.12 } },
      },
      tooltip: {
        trigger: "item",
        formatter: (params: { data: { name: string; exact: string[] } }) =>
          `<strong>${params.data.name}</strong><br>${front.axes[0].label}: ${params.data.exact[0]}<br>${front.axes[1].label}: ${params.data.exact[1]}`,
      },
      series: [{
        type: "scatter",
        data: front.points.map((point) => ({
          name: point.label,
          // Numeric conversion is visualization-only; DTO values and tooltips stay exact text.
          value: [Number(point.values[0]), Number(point.values[1])],
          exact: point.values,
          symbolSize: point.selected ? 20 : 13,
          itemStyle: {
            color: point.selected ? selectedColor : peerColor,
            borderColor: backgroundColor,
            borderWidth: 2,
          },
        })),
      }],
    });
    chart.on("click", (params) => {
      const data = params.data;
      if (data && typeof data === "object" && "exact" in data && Array.isArray(data.exact)) {
        onSelect?.(data.exact.filter((value): value is string => typeof value === "string"));
      }
    });
    let pendingResize = 0;
    const resize = new ResizeObserver(() => {
      window.cancelAnimationFrame(pendingResize);
      pendingResize = window.requestAnimationFrame(() => chart.resize());
    });
    resize.observe(container.current);
    return () => {
      resize.disconnect();
      window.cancelAnimationFrame(pendingResize);
      chart.dispose();
    };
  }, [front, onSelect]);
  if (!front) {
    return <div className="empty-state">This outcome has no tradeoff curve.</div>;
  }
  return <>
    <div className="analysis-meta">
      <span>{front.completeness} frontier</span>
      <strong>{front.points.length} non-dominated outcomes</strong>
    </div>
    <div ref={container} className="pareto-chart" aria-label="Pareto frontier chart" />
    {front.axes.length > 2 && <div className="pareto-dimensions">
      {front.points.map((point) => <button type="button" key={point.label} className={point.selected ? "selected" : ""} onClick={() => onSelect?.(point.values)}>
        <strong>{point.label}</strong>
        {front.axes.map((axis, index) => <span key={axis.key}>{axis.label}: {point.values[index]}</span>)}
      </button>)}
    </div>}
  </>;
}
