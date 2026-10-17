<script setup lang="ts">
import { useData } from "vitepress";
import DocsShell from "./components/DocsShell.vue";

const { frontmatter } = useData();
</script>

<template>
  <Content v-if="frontmatter.layout === false" />
  <DocsShell v-else />
</template>
