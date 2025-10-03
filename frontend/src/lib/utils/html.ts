/**
 * サーバーサイドで二重にエスケープされたHTMLエンティティを一段階デコードする。
 * これにより、`&amp;gt;` が `&gt;` になり、`{@html}` で正しく `>` として表示される。
 * @param html エスケープされたHTML文字列
 */
export function fixDoubleEscaping(html: string | null | undefined): string {
	if (!html) return '';
	// 二重にエスケープされた `&`, `<`, `>` を元に戻す
	return html.replace(/&amp;gt;/g, '&gt;').replace(/&amp;lt;/g, '&lt;').replace(/&amp;amp;/g, '&amp;');
}

/**
 * HTMLエンティティをデコードするヘルパー関数。
 * サーバーサイドレンダリング(SSR)中は基本的な置換のみ行い、
 * ブラウザ環境ではDOMパーサーを利用して正確にデコードします。
 * @param text デコードするHTML文字列
 */
export function decodeHtmlEntities(text: string | null | undefined): string {
	if (!text) return '';
	if (typeof document !== 'undefined') {
		const textarea = document.createElement('textarea');
		textarea.innerHTML = text;
		return textarea.value;
	}
	// SSR用の基本的なフォールバック(&amp;は最後に置換)
	return text.replace(/&gt;/g, '>').replace(/&lt;/g, '<').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');
}
