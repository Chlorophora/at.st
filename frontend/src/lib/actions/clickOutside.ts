/**
 * use:clickOutside アクション
 * このアクションが適用された要素の外側がクリックされたときに 'outclick' イベントを発行します。
 * @param node - アクションが適用されるHTML要素
 */
export function clickOutside(node: HTMLElement) {
	const handleClick = (event: MouseEvent) => {
		const target = event.target as Node;

		// クリックされた要素が、現在のアクションが適用されているノードの内部にある場合は何もしない
		if (node && node.contains(target)) {
			return;
		}

		// クリックされた要素が、他のポップアップ（.popover または .backlinks-popover）の内部にある場合、
		// かつ、それが現在のアクションが適用されているノード（node）の子孫ではない場合にのみ、何もしない。
		// これにより、子ポップアップをクリックしても親は閉じないが、
		// 親ポップアップ内の要素（例：レスアンカー）から新しいポップアップを開くことは可能になる。
		if ((target as HTMLElement).closest('.popover, .backlinks-popover, .id-posts-popover') && !node.contains(target)) {
			return;
		}

		// 上記の条件に当てはまらず、イベントがまだ処理されていない場合のみoutclickイベントを発行
		if (!event.defaultPrevented) {
			node.dispatchEvent(new CustomEvent('outclick'));
		}
	};

	document.addEventListener('click', handleClick, true); // キャプチャフェーズでイベントを捕捉

	return {
		destroy() {
			document.removeEventListener('click', handleClick, true);
		}
	};
}