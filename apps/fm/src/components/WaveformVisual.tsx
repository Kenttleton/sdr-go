/**
 * WaveformVisual.tsx
 *
 * Full-bleed background visual rendered via Skia.
 * Two modes:
 *   'artistic'     — ambient aurora: mirrored gradient bands that breathe with audio
 *   'oscilloscope' — raw waveform trace, phosphor CRT style
 *
 * Sits behind all UI as a pure visual layer (pointerEvents="none").
 */

import React, { useMemo, useEffect, useRef } from "react";
import { Animated } from "react-native";
import {
  Canvas,
  Path,
  LinearGradient,
  vec,
  Rect,
  Paint,
  Skia,
  BlurMask,
  Group,
} from "@shopify/react-native-skia";
import type { Theme } from "@sdrgo/ui-core";

interface Props {
  data: number[]; // 512 float samples, -1..1
  mode: "artistic" | "oscilloscope";
  theme: Theme;
  isPlaying: boolean;
  width: number;
  height: number;
}

// ── Build SVG-style path strings for Skia ────────────────────────────────────

function buildOscilloscopePath(
  data: number[],
  width: number,
  height: number,
): string {
  if (!data.length) return "";
  const mid = height / 2;
  const xStep = width / (data.length - 1);
  let d = `M 0 ${mid + data[0] * mid * 0.85}`;
  for (let i = 1; i < data.length; i++) {
    const x = i * xStep;
    const y = mid + data[i] * mid * 0.85;
    d += ` L ${x.toFixed(1)} ${y.toFixed(1)}`;
  }
  return d;
}

function buildMirroredEnvelope(
  data: number[],
  width: number,
  height: number,
  scale: number = 1,
): { top: string; bottom: string } {
  const mid = height / 2;
  const windowSize = 20;
  const n = data.length;
  const xStep = width / (n - 1);

  const envelope: number[] = [];
  for (let i = 0; i < n; i++) {
    const s = Math.max(0, i - windowSize);
    const e = Math.min(n - 1, i + windowSize);
    let max = 0;
    for (let j = s; j <= e; j++) max = Math.max(max, Math.abs(data[j]));
    envelope.push(max);
  }

  let top = `M 0 ${mid}`;
  let bottom = `M 0 ${mid}`;
  for (let i = 0; i < n; i++) {
    const x = (i * xStep).toFixed(1);
    const amp = envelope[i] * mid * 0.75 * scale;
    top += ` L ${x} ${(mid - amp).toFixed(1)}`;
    bottom += ` L ${x} ${(mid + amp).toFixed(1)}`;
  }
  top += ` L ${width} ${mid} Z`;
  bottom += ` L ${width} ${mid} Z`;

  return { top, bottom };
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function WaveformVisual({
  data,
  mode,
  theme,
  isPlaying,
  width,
  height,
}: Props) {
  // Animated idle breath when no audio
  const breathAnim = useRef(new Animated.Value(0)).current;
  const breathRef = useRef(0);

  useEffect(() => {
    const anim = Animated.loop(
      Animated.sequence([
        Animated.timing(breathAnim, {
          toValue: 1,
          duration: 3200,
          useNativeDriver: false,
        }),
        Animated.timing(breathAnim, {
          toValue: 0,
          duration: 3200,
          useNativeDriver: false,
        }),
      ]),
    );
    anim.start();
    const id = breathAnim.addListener(({ value }) => {
      breathRef.current = value;
    });
    return () => {
      anim.stop();
      breathAnim.removeListener(id);
    };
  }, []);

  // ── Paths ─────────────────────────────────────────────────────────────────
  const hasSignal = isPlaying && data.some((v) => v !== 0);

  const oscPath = useMemo(() => {
    if (mode !== "oscilloscope") return "";
    return buildOscilloscopePath(data, width, height);
  }, [data, mode, width, height]);

  const { envelopeTop, envelopeBottom } = useMemo(() => {
    if (mode !== "artistic") return { envelopeTop: "", envelopeBottom: "" };
    const scale = hasSignal
      ? 1
      : 0.04 + 0.03 * Math.sin((Date.now() / 3200) * Math.PI);
    const { top, bottom } = buildMirroredEnvelope(data, width, height, scale);
    return { envelopeTop: top, envelopeBottom: bottom };
  }, [data, mode, width, height, hasSignal]);

  // Idle (flat line breathing) for when no audio
  const idleScale = hasSignal ? 1 : 0.04;
  const { envelopeTop: idleTop, envelopeBottom: idleBottom } = useMemo(() => {
    const flatData = Array(512)
      .fill(0)
      .map((_, i) => Math.sin((i / 512) * Math.PI * 6) * 0.3);
    const { top, bottom } = buildMirroredEnvelope(
      flatData,
      width,
      height,
      idleScale,
    );
    return { envelopeTop: top, envelopeBottom: bottom };
  }, [width, height, idleScale]);

  const activePath = hasSignal ? envelopeTop : idleTop;
  const activeBottomPath = hasSignal ? envelopeBottom : idleBottom;

  // ── Artistic colour scheme from theme ─────────────────────────────────────
  // Deep fade: transparent at edges, rich in center-vertical band
  const midY = height * 0.5;
  const gradTop = height * 0.15;
  const gradBot = height * 0.85;

  return (
    <Canvas style={{ width, height }}>
      {/* Base: very subtle background gradient for depth */}
      <Rect x={0} y={0} width={width} height={height}>
        <LinearGradient
          start={vec(0, 0)}
          end={vec(0, height)}
          colors={[theme.background, theme.surfaceMid + "88", theme.background]}
        />
      </Rect>

      {mode === "artistic" && (
        <Group>
          {/* Glow layer — wide, blurred, very transparent */}
          <Path path={activePath} color={theme.primary + "18"} style="fill">
            <BlurMask blur={40} style="normal" />
          </Path>
          <Path
            path={activeBottomPath}
            color={theme.primary + "18"}
            style="fill"
          >
            <BlurMask blur={40} style="normal" />
          </Path>

          {/* Mid glow — tighter, slightly more opaque */}
          <Path path={activePath} color={theme.primary + "28"} style="fill">
            <BlurMask blur={16} style="normal" />
          </Path>
          <Path
            path={activeBottomPath}
            color={theme.primary + "28"}
            style="fill"
          >
            <BlurMask blur={16} style="normal" />
          </Path>

          {/* Signal accent — sharp inner edge */}
          <Path path={activePath} color={theme.signal + "1a"} style="fill">
            <BlurMask blur={6} style="normal" />
          </Path>
          <Path
            path={activeBottomPath}
            color={theme.signal + "1a"}
            style="fill"
          >
            <BlurMask blur={6} style="normal" />
          </Path>
        </Group>
      )}

      {mode === "oscilloscope" && oscPath !== "" && (
        <Group>
          {/* Phosphor glow */}
          <Path
            path={oscPath}
            color={theme.signal + "44"}
            style="stroke"
            strokeWidth={6}
          >
            <BlurMask blur={8} style="normal" />
          </Path>
          {/* Sharp trace */}
          <Path
            path={oscPath}
            color={theme.signal + "cc"}
            style="stroke"
            strokeWidth={1.5}
          />
        </Group>
      )}

      {/* Vertical fade vignette — darkens top and bottom so UI text stays legible */}
      <Rect x={0} y={0} width={width} height={height}>
        <LinearGradient
          start={vec(0, 0)}
          end={vec(0, height)}
          colors={[
            theme.background + "ff",
            theme.background + "00",
            theme.background + "00",
            theme.background + "ff",
          ]}
          positions={[0, 0.22, 0.78, 1]}
        />
      </Rect>
    </Canvas>
  );
}
