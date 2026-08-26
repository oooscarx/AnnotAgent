import { useId } from "react";

export type AnnotAgentLogoProps = {
  compact?: boolean;
  darkSurface?: boolean;
  product?: "core" | "robocup";
  className?: string;
  title?: string;
};

export function AnnotAgentLogo({
  compact = false,
  darkSurface = false,
  product = "core",
  className,
  title = product === "robocup" ? "RoboCup AnnotAgent" : "AnnotAgent",
}: AnnotAgentLogoProps) {
  const rawId = useId();
  const gradientId = `aa-gradient-${rawId.replace(/:/g, "")}`;
  const ink = darkSurface ? "#F8FAFC" : "#0D1117";
  const muted = darkSurface ? "#94A3B8" : "#64748B";

  return (
    <svg
      className={className}
      viewBox={compact ? "0 0 128 128" : product === "robocup" ? "0 0 700 136" : "0 0 590 128"}
      role="img"
      aria-label={title}
    >
      <title>{title}</title>
      <defs>
        <linearGradient id={gradientId} x1="24" y1="20" x2="108" y2="108" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#00B3A4" />
          <stop offset="0.48" stopColor="#2563EB" />
          <stop offset="1" stopColor="#7C3AED" />
        </linearGradient>
      </defs>
      <Mark fill={`url(#${gradientId})`} ink={ink} />
      {!compact && product === "core" && (
        <>
          <text x="151" y="81" fontFamily="Inter, ui-sans-serif, system-ui, sans-serif" fontSize="61" fontWeight="750" letterSpacing="-2.3">
            <tspan fill={ink}>Annot</tspan><tspan fill="#00B3A4">Agent</tspan>
          </text>
          <text x="154" y="108" fontFamily="Inter, ui-sans-serif, system-ui, sans-serif" fontSize="15" fontWeight="500" letterSpacing=".4" fill={muted}>
            AGENTIC ANNOTATION · AUDITABLE BY DESIGN
          </text>
        </>
      )}
      {!compact && product === "robocup" && (
        <>
          <text x="155" y="47" fontFamily="Inter, ui-sans-serif, system-ui, sans-serif" fontSize="23" fontWeight="700" letterSpacing="1.6" fill={ink}>ROBOCUP</text>
          <text x="153" y="96" fontFamily="Inter, ui-sans-serif, system-ui, sans-serif" fontSize="57" fontWeight="750" letterSpacing="-2.2">
            <tspan fill={ink}>Annot</tspan><tspan fill="#00B3A4">Agent</tspan>
          </text>
          <text x="155" y="123" fontFamily="Inter, ui-sans-serif, system-ui, sans-serif" fontSize="15" fontWeight="500" fill={muted}>
            VLM-assisted annotation and quality control for robot soccer perception
          </text>
        </>
      )}
    </svg>
  );
}

function Mark({ fill, ink }: { fill: string; ink: string }) {
  return (
    <g>
      <g fill="none" strokeLinecap="round" strokeLinejoin="round">
        <path d="M33 14H22a8 8 0 0 0-8 8v11" stroke={ink} strokeWidth="7" />
        <path d="M95 14h11a8 8 0 0 1 8 8v11" stroke={ink} strokeWidth="7" />
        <path d="M14 95v11a8 8 0 0 0 8 8h11" stroke="#00B3A4" strokeWidth="7" />
        <path d="M114 95v11a8 8 0 0 1-8 8H95" stroke="#7C3AED" strokeWidth="7" />
      </g>
      <path fill={fill} fillRule="evenodd" d="M36 99 59.5 31.5C61.2 26.6 64.6 24 69.8 24h5.1c5.1 0 8.6 2.7 10.3 7.5L109 99H86.4l-6.7-20.3H54.4L47.9 99H36Zm25.1-41.1h12.2l-6-18.1-6.2 18.1Z" />
      <path d="M55.3 77.8h23.6" stroke="rgba(255,255,255,.72)" strokeWidth="3" strokeLinecap="round" />
    </g>
  );
}
