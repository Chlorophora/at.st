import adapter from '@sveltejs/adapter-node';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

const config = {
    preprocess: vitePreprocess({ script: true }),

    kit: {
        adapter: adapter(),
        csrf: {
            checkOrigin: false
        },
    }
};

export default config;
