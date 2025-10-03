<script lang="ts">
	export let text: string;
	export let onAnchorClick: (event: MouseEvent, responseNumber: number) => void;

	// Use `matchAll` for the most robust and stateless parsing.
	$: parsedBody = (() => {
		const parts: { type: 'text' | 'anchor'; content: string | number }[] = [];
		if (!text) return [];

		const anchorRegex = />>(\d+)/g; // Must be global for matchAll
		const matches = [...text.matchAll(anchorRegex)];
		let lastIndex = 0;

		for (const match of matches) {
			// Text before the anchor
			if (match.index > lastIndex) {
				parts.push({ type: 'text', content: text.slice(lastIndex, match.index) });
			}
			// The anchor itself
			const responseNumber = parseInt(match[1], 10);
			parts.push({ type: 'anchor', content: responseNumber });
			lastIndex = match.index + match[0].length;
		}

		// Text after the last anchor
		if (lastIndex < text.length) {
			parts.push({ type: 'text', content: text.slice(lastIndex) });
		}

		return parts;
	})();
</script>

{#each parsedBody as part}
	{#if part.type === 'text'}{part.content}{:else if part.type === 'anchor'}<a
			href="#"
			class="response-anchor"
			on:click|preventDefault={(e) => onAnchorClick(e, part.content as number)}
		>&gt;&gt;{part.content}</a>{/if}
{/each}

<style>
	.response-anchor {
		color: #007bff;
		text-decoration: underline;
		cursor: pointer;
	}
	.response-anchor:hover {
		color: #0056b3;
	}
</style>