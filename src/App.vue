<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import StickyWindow from "./components/StickyWindow.vue";
import TaskbarPinned from "./components/TaskbarPinned.vue";
import PopupList from "./components/PopupList.vue";
import TrashView from "./components/TrashView.vue";
import ToastManager from "./components/ToastManager.vue";
import { useConfigStore } from "./stores/configStore";

function getRoute() {
  const hash = window.location.hash.replace(/^#\/?/, "");
  return hash || "sticky";
}

const route = ref(getRoute());
const configStore = useConfigStore();

function updateRoute() {
  route.value = getRoute();
}

onMounted(async () => {
  window.addEventListener("hashchange", updateRoute);
  await configStore.load();
});

watch(() => configStore.config.general.fontFamily, (font) => {
  if (font) document.documentElement.style.fontFamily = font;
}, { immediate: true });

onBeforeUnmount(() => window.removeEventListener("hashchange", updateRoute));
</script>

<template>
  <StickyWindow v-if="route === 'sticky'" />
  <TaskbarPinned v-else-if="route === 'taskbar'" />
  <PopupList v-else-if="route === 'popup'" />
  <TrashView v-else-if="route === 'trash'" />
  <ToastManager />
</template>
