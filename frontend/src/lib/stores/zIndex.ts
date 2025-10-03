/**
 * Creates a manager for z-index values for floating elements.
 * Each time a new z-index is requested, it increments the current value.
 */
function createZIndexManager() {
	// Start with a base z-index high enough to be on top of most page content.
	let currentZIndex = 1050;

	return {
		/**
		 * Gets a new, unique, and higher z-index value.
		 * @returns {number} The new z-index.
		 */
		getNewZIndex: (): number => {
			currentZIndex += 1;
			return currentZIndex;
		}
	};
}

export const zIndexManager = createZIndexManager();
