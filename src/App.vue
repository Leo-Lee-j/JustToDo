<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import StickyWindow from "./components/StickyWindow.vue";
import PopupList from "./components/PopupList.vue";
import ToastManager from "./components/ToastManager.vue";
import { useConfigStore } from "./stores/configStore";
import { useTaskStore } from "./stores/taskStore";

function getRoute() {
  const hash = window.location.hash.replace(/^#\/?/, "");
  return hash || "sticky";
}

const route = ref(getRoute());
const configStore = useConfigStore();
const taskStore = useTaskStore();

function updateRoute() {
  route.value = getRoute();
}

onMounted(async () => {
  window.addEventListener("hashchange", updateRoute);
  await Promise.all([configStore.load(), taskStore.load()]);
});

watch(() => configStore.config.general.fontFamily, (font) => {
  if (font) document.documentElement.style.fontFamily = font;
}, { immediate: true });

onBeforeUnmount(() => window.removeEventListener("hashchange", updateRoute));
</script>

<template>
  <StickyWindow v-if="route === 'sticky'" />
  <PopupList v-else-if="route === 'popup'" />
  <ToastManager />
</template>
