<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vitepress";
import Icon from "./Icon.vue";
import type { NavItem } from "../types";
import { useLinks } from "../links";

const props = defineProps<{ item: NavItem; hash: string }>();
const route = useRoute();
const { to, samePage } = useLinks();

function contains(item: NavItem): boolean {
  return samePage(item.link, route.path) || (item.items ?? []).some(contains);
}

function selected(link: string) {
  if (!samePage(link, route.path)) return false;
  const want = link.includes("#") ? link.slice(link.indexOf("#")) : "";
  return want === props.hash || (!want && !props.hash);
}

const isOpen = computed(() => contains(props.item));
</script>

<template>
  <details v-if="item.items" :open="isOpen">
    <summary><Icon name="chevron" class="feather-chevron-right" /><span>{{ item.text }}</span></summary>
    <div>
      <a :href="to(item.link)" :class="{ selected: selected(item.link) }">
        <Icon :name="item.icon ?? 'file'" /><span>{{ item.linkText ?? item.text }}</span>
      </a>
      <NavItem v-for="child in item.items" :key="child.link" :item="child" :hash="hash" />
    </div>
  </details>
  <a v-else :href="to(item.link)" :class="{ selected: selected(item.link) }" :aria-current="selected(item.link) ? 'page' : undefined">
    <Icon :name="item.icon ?? 'bookmark'" /><span>{{ item.text }}</span>
  </a>
</template>
