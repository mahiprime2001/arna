<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import QRCode from "qrcode";

// Render `value` to a QR code on a canvas. Re-renders when value/size change.
const props = withDefaults(defineProps<{ value: string; size?: number }>(), { size: 160 });
const canvas = ref<HTMLCanvasElement | null>(null);

async function render() {
  if (!canvas.value || !props.value) return;
  try {
    await QRCode.toCanvas(canvas.value, props.value, {
      width: props.size,
      margin: 1,
      color: { dark: "#0b0e14", light: "#ffffff" },
    });
  } catch {
    /* ignore render errors */
  }
}

onMounted(render);
watch(() => [props.value, props.size], render);
</script>

<template>
  <canvas ref="canvas" class="rounded-lg" :width="size" :height="size" />
</template>
