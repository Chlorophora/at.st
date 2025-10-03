import { readable, writable } from 'svelte/store';
import { browser } from '$app/environment';

/**
 * レス投稿パネルの表示状態を管理するストア。
 * trueなら表示、falseなら非表示。
 */
export const isCommentFormPanelOpen = writable(false);

/**
 * レス投稿パネルを開きます。
 */
export function openCommentFormPanel() {
	isCommentFormPanelOpen.set(true);
}

/**
 * レス投稿パネルを閉じます。
 */
export function closeCommentFormPanel() {
	isCommentFormPanelOpen.set(false);
}

/**
 * スレッド投稿パネルの表示状態を管理するストア。
 * trueなら表示、falseなら非表示。
 */
export const isThreadFormPanelOpen = writable(false);

/**
 * スレッド投稿パネルを開きます。
 */
export function openThreadFormPanel() {
	isThreadFormPanelOpen.set(true);
}

/**
 * スレッド投稿パネルを閉じます。
 */
export function closeThreadFormPanel() {
	isThreadFormPanelOpen.set(false);
}

/**
 * 現在の画面がモバイルサイズ（768px以下）かどうかを判定するストア。
 * サーバーサイドでは常に false。
 */
export const isMobile = readable(false, (set) => {
	if (!browser) {
		return;
	}

	const mediaQuery = window.matchMedia('(max-width: 768px)');

	function update(e: MediaQueryListEvent | MediaQueryList) {
		set(e.matches);
	}

	mediaQuery.addEventListener('change', update);
	update(mediaQuery); // 初期値を設定

	return () => {
		mediaQuery.removeEventListener('change', update);
	};
});