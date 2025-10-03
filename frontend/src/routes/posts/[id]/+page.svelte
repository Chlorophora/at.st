<script lang="ts">
	import type { PageData } from './$types';
	import type { Post, Comment, IdentityDetails } from 'src/app';
	import { goto } from '$app/navigation';
	import BanModal from '$lib/components/BanModal.svelte';
	import { onDestroy, onMount, tick } from 'svelte';
	import { currentThreadResponseCount } from '$lib/stores/thread';
	import { fly } from 'svelte/transition';
	import { page } from '$app/stores';
	import { browser } from '$app/environment';
	import { openCommentFormPanel, closeCommentFormPanel, isMobile } from '$lib/stores/ui';
	import CommentFormPanel from '$lib/components/CommentFormPanel.svelte';
	import { decodeHtmlEntities, fixDoubleEscaping } from '$lib/utils/html';
	import BacklinksPopover from '$lib/components/BacklinksPopover.svelte';
	import ResponsePopover from '$lib/components/ResponsePopover.svelte';
	import IdPostsPopover from '$lib/components/IdPostsPopover.svelte';

	export let data: PageData;

	// --- State Variables ---

	// サイドパネルの開閉状態を管理
	let isPanelOpen = true;
	// --- パネルリサイズ機能 ---
	let panelWidth = 380; // デフォルトのパネル幅
	let isResizing = false;
	let startX: number;
	let startWidth: number;

	// --- レスアンカーポップアップ機能 ---
	type PopoverState = {
		id: string;
		response: Post | Comment;
		position: { x: number; y: number };
		responseNumber: number;
		level: number;
	};
	let popoverTrees = new Map<string, PopoverState[]>();
	let backlinksMap = new Map<number, Set<number>>(); // 各レスへの参照元レス番号を格納
	let popoverCloseTimers = new Map<string, number>(); // ポップアップを閉じるタイマーIDを管理

	// --- 被参照元一覧ポップアップ機能 ---
	type BacklinksPopoverState = {
		id: string;
		level: number;
		targetResponseNumber: number;
		backlinks: { response: Post | Comment; number: number }[];
		position: { x: number; y: number };
	};
	let backlinksPopovers: BacklinksPopoverState[] = [];

	// --- ID投稿一覧ポップアップ機能 ---
	type IdPostsPopoverState = {
		id: string;
		banType: 'User' | 'Ip' | 'Device';
		idPartValue: string;
		position: { x: number; y: number };
		sourceId: number;
		canModerate: boolean;
	};
	let idPostsPopover: IdPostsPopoverState | null = null;

	// --- Mobile Modal State ---
	type MobileModalState = {
		type: 'response' | 'backlinks' | 'id-posts';
		props: any;
		// id for {#key} block to force re-render on stack change
		id: number;
	};
	let mobileModalStack: MobileModalState[] = [];

	// --- Event Handling Flags ---

	// BAN Modal
	let isBanModalOpen = false;
	let banTarget: {
		type: 'post' | 'comment';
		id: number;
		banType?: 'User' | 'Ip' | 'Device';
		context?: { boardId?: number; postId?: number };
	} | null = null;
	let identityDetails: IdentityDetails | null = null;

	// Sorting
	let sortOrder = 'momentum_desc';
	const sortOptions = [
		{ value: 'momentum_desc', label: '勢い (降順)' },
		{ value: 'momentum_asc', label: '勢い (昇順)' },
		{ value: 'responses_desc', label: 'レス数 (降順)' },
		{ value: 'responses_asc', label: 'レス数 (昇順)' },
		{ value: 'last_activity_desc', label: '最終更新 (降順)' },
		{ value: 'last_activity_asc', label: '最終更新 (昇順)' },
		{ value: 'created_at_desc', label: '作成日時 (降順)' },
		{ value: 'created_at_asc', label: '作成日時 (昇順)' }
	];

	// --- Reactive Statements for Auth ---
	$: isAdmin = $page.data.user?.role === 'admin';

	// --- Store Updates ---

	// このページが持っているコメントの配列 (data.comments) を監視し、
	// その個数（＝総レス数）を共有ストアにセットします。
	// これにより、ヘッダーコンポーネントがレス数をリアルタイムで受け取れます。
	$: if (data && data.comments) {
		// 総レス数は、元の投稿(1) + コメントの数
		currentThreadResponseCount.set(data.comments.length + 1);
	}

	// ユーザーがこのページから離れる際に、ストアの値をリセットします。
	// これにより、他のページでレス数が誤って表示されるのを防ぎます。
	onDestroy(() => {
		currentThreadResponseCount.set(null);
	});

	// --- Helper Functions ---

	// Panel Resizing
	function handleMouseDown(event: MouseEvent) {
		isResizing = true;
		startX = event.clientX;
		startWidth = panelWidth;
		document.body.style.cursor = 'col-resize';
		document.body.style.userSelect = 'none';
		window.addEventListener('mousemove', handleMouseMove);
		window.addEventListener('mouseup', handleMouseUp);
	}

	function handleMouseMove(event: MouseEvent) {
		if (!isResizing) return;
		const dx = event.clientX - startX;
		const newWidth = startWidth + dx;
		const minWidth = 280;
		const maxWidth = document.documentElement.clientWidth * 0.7;
		panelWidth = Math.max(minWidth, Math.min(newWidth, maxWidth));
	}

	function handleMouseUp() {
		isResizing = false;
		document.body.style.cursor = '';
		document.body.style.userSelect = '';
		window.removeEventListener('mousemove', handleMouseMove);
		window.removeEventListener('mouseup', handleMouseUp);
	}

	// Popover Logic (Centralized)
	function findResponse(responseNumber: number): Post | Comment | null {
		if (!data.post || !data.comments) return null;
		if (responseNumber === 1) return data.post;
		if (responseNumber > 1 && responseNumber <= data.comments.length + 1) {
			return data.comments[responseNumber - 2];
		}
		return null;
	}

	async function showPopoverFor(anchorElement: HTMLAnchorElement, treeId: string, level: number) {
		try { // try-finallyは残して、エラー時もロックが解除されるようにする
			const href = anchorElement.getAttribute('href');
			const match = href?.match(/#res-(\d+)/);
			if (!match?.[1]) return;
			const responseNumber = parseInt(match[1], 10);
			const targetResponseData = findResponse(responseNumber);

			if (!targetResponseData) return;

			const newPopoverId = `popover-${treeId}-${level}`;
			const newPopover: PopoverState = {
				id: newPopoverId,
				response: targetResponseData,
				responseNumber: responseNumber,
				position: { x: -9999, y: -9999 }, // 一時的に画面外へ
				level,
			};

			const currentTree = popoverTrees.get(treeId) || [];
			// 既存のツリーから、新しいポップアップと同じかそれより深いレベルのものを削除
			const baseTree = currentTree.filter(p => p.level < level);
			const newTree = [...baseTree, newPopover];

			popoverTrees.set(treeId, newTree);
			popoverTrees = new Map(popoverTrees); // Svelteに更新を通知

			await tick(); // DOM更新を待つ

			const popoverEl = document.getElementById(newPopoverId);
			// 親要素はメインページのレス、ResponsePopover、またはBacklinksPopover内のレス項目
			const parentEl = anchorElement.closest('.response, .response-item, .popover, .backlinks-popover');

			if (!popoverEl || !parentEl) {
				popoverTrees.set(treeId, currentTree.filter(p => p.id !== newPopoverId));
				popoverTrees = new Map(popoverTrees);
				return;
			}

			const anchorRect = anchorElement.getBoundingClientRect();
			const popoverRect = popoverEl.getBoundingClientRect();
			const parentRect = parentEl.getBoundingClientRect();
			const viewportWidth = window.innerWidth;
			const viewportHeight = window.innerHeight;
			const margin = 10;
			const offset = 5;

			let x: number;
			let y = parentRect.top - popoverRect.height - offset;

			const parentResponseNumberEl = parentEl.querySelector('.response-number');
			const popoverResponseNumberEl = popoverEl.querySelector('.response-number');

			if (parentResponseNumberEl && popoverResponseNumberEl) {
				const parentNumRect = parentResponseNumberEl.getBoundingClientRect();
				const popoverNumRect = popoverResponseNumberEl.getBoundingClientRect();
				const popoverNumOffsetX = popoverNumRect.left - popoverRect.left;
				x = parentNumRect.left - popoverNumOffsetX - 10;
			} else {
				x = anchorRect.left + anchorRect.width / 2 - popoverRect.width / 2;
			}
			
			if (y < margin) {
				y = parentRect.bottom + offset;
			}
			if (x < margin) {
				x = margin;
			}
			if (x + popoverRect.width > viewportWidth - margin) {
				x = viewportWidth - popoverRect.width - margin;
			}

			const finalTree = popoverTrees.get(treeId);
			if (!finalTree) return;
			const popoverIndex = finalTree.findIndex((p) => p.id === newPopoverId);
			if (popoverIndex === -1) return;

			finalTree[popoverIndex].position = { x, y };
			popoverTrees = new Map(popoverTrees); // Svelteに更新を通知
		} finally {
			// popoverOperationLock を削除
		}
	}

	function handlePopoverMouseEnter(event: CustomEvent<{ treeId: string }>) {
		const { treeId } = event.detail;
		const timerId = popoverCloseTimers.get(treeId);
		if (timerId) {
			clearTimeout(timerId);
			popoverCloseTimers.delete(treeId);
		}
	}

	function handlePopoverMouseLeave(event: CustomEvent<{ treeId: string }>) {
		const { treeId } = event.detail;
		const timerId = setTimeout(() => {
			const tree = popoverTrees.get(treeId);
			if (tree) {
				popoverTrees.delete(treeId);
				popoverTrees = new Map(popoverTrees);
			}
		}, 500);
		popoverCloseTimers.set(treeId, timerId);
	}

	function closeAllPopovers() {
		popoverTrees.clear();
		backlinksPopovers = [];
		idPostsPopover = null;
	}

	// マウスオーバーによるポップアップ表示/非表示を管理するグローバルなハンドラ
	function handleGlobalMouseOver(event: MouseEvent) {
		const target = event.target as HTMLElement;
		// モバイルではマウスオーバーでのポップアップは無効
		if ($isMobile) return;
		
		// 既存のタイマーをクリアする
		for (const [treeId, timerId] of popoverCloseTimers.entries()) {
			const associatedPopover = document.querySelector(`[data-tree-id="${treeId}"]`);
			// ポップアップ自体にホバーしていなければタイマーをクリア
			if (!associatedPopover || !associatedPopover.matches(':hover')) {
				clearTimeout(timerId);
				popoverCloseTimers.delete(treeId);
			}
		}

		// 1. レスアンカーの上か？
		const anchor = target.closest('a.response-anchor');
		if (anchor) {
			const parentElement = anchor.closest('.response');
			// メインコンテンツのレスアンカー以外は無視（ポップアップ内のアンカーはクリックで処理）
			if (!parentElement) return;

			// 新しいツリーを開始
			const treeId = `tree-${Date.now()}`;
			const currentTree = popoverTrees.get(treeId) || [];
			if (currentTree.length > 0) return; // 既にこのツリーに何かあれば何もしない

			// このアンカーに関連するクローズタイマーがあればクリア
			const timerId = popoverCloseTimers.get(treeId);
			if (timerId) {
				clearTimeout(timerId);
				popoverCloseTimers.delete(treeId);
			}

			showPopoverFor(anchor as HTMLAnchorElement, treeId, 0);
			return;
		}

		// 2. いずれかのポップアップの上か？
		const closestPopover = target.closest('.popover, .backlinks-popover, .id-posts-popover');
		if (closestPopover) {
			const treeId = closestPopover.id.split('-').slice(1, -1).join('-');
			// ポップアップ上にマウスがある場合は、そのツリーのクローズタイマーをキャンセル
			const timerId = popoverCloseTimers.get(treeId);
			if (timerId) {
				clearTimeout(timerId);
				popoverCloseTimers.delete(treeId);
			}
			return;
		}

		// 3. どのポップアップツリーの上にもない場合、開いているツリーを閉じるタイマーを開始
		for (const treeId of popoverTrees.keys()) {
			if (!popoverCloseTimers.has(treeId)) {
				const timerId = setTimeout(() => {
					popoverTrees.delete(treeId);
					popoverTrees = new Map(popoverTrees);
				}, 500);
				popoverCloseTimers.set(treeId, timerId);
			}
		}
	}


	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			// BANモーダルが開いていれば最優先で閉じる
			if (isBanModalOpen) {
				isBanModalOpen = false;
				event.preventDefault();
				return;
			}

			if (idPostsPopover) {
				idPostsPopover = null;
				event.preventDefault();
				return;
			}

			if (backlinksPopovers.length > 0) {
				backlinksPopovers = backlinksPopovers.slice(0, -1);
				event.preventDefault(); // 他のEscapeキーイベントを抑制
				return;
			}

			// 開いている一番深いResponsePopoverを閉じる
			let deepestTreeId: string | null = null;
			let deepestLevel = -1;
			for (const [treeId, tree] of popoverTrees.entries()) {
				if (tree.length -1 > deepestLevel) {
					deepestLevel = tree.length - 1;
					deepestTreeId = treeId;
				}
			}
			if (deepestTreeId) {
				const tree = popoverTrees.get(deepestTreeId)!;
				tree.pop();
				if (tree.length === 0) {
					popoverTrees.delete(deepestTreeId);
				}
				popoverTrees = new Map(popoverTrees);
			}
		}
	}

	// --- Mobile Modal Logic ---
	function showResponseModal(responseNumber: number) {
		const response = findResponse(responseNumber);
		if (!response) return;
		mobileModalStack = [
			...mobileModalStack,
			{
				type: 'response',
				id: Date.now(),
				props: {
					response,
					responseNumber,
					backlinkCount: backlinkCounts.get(responseNumber)
				}
			}
		];
	}

	function showIdPostsModal(banType: 'User' | 'Ip' | 'Device', idPartValue: string, sourceId: number) {
		mobileModalStack = [
			...mobileModalStack,
			{
				type: 'id-posts',
				id: Date.now(),
				props: {
					banType,
					idPartValue,
					posts: [data.post, ...data.comments],
					sourceId,
					// このスレッドに対するモデレート権限を渡す
					canModerate: data.post?.can_moderate ?? false,
					isAdmin
				}
			}
		];
	}

	function showBacklinksModal(targetResponseNumber: number) {
		const sourceNumbers = backlinksMap.get(targetResponseNumber);
		if (!sourceNumbers || sourceNumbers.size === 0) return;
		const backlinkData = [...sourceNumbers]
			.map((num) => ({ response: findResponse(num), number: num }))
			.filter((item): item is { response: Post | Comment; number: number } => item.response !== null);
		if (backlinkData.length === 0) return;

		mobileModalStack = [
			...mobileModalStack,
			{
				type: 'backlinks',
				id: Date.now(),
				props: {
					backlinks: backlinkData,
					targetResponseNumber,
					backlinkCounts
				}
			}
		];
	}

	function closeTopMobileModal() {
		if (mobileModalStack.length > 0) {
			mobileModalStack = mobileModalStack.slice(0, -1);
		}
	}

	function handleShowResponse(e: CustomEvent<{ responseNumber: number; event: MouseEvent, level?: number }>) {
		if ($isMobile) {
			showResponseModal(e.detail.responseNumber);
		} else {
			// クリックによる表示。マウスオーバーとは別のツリーとして扱う。
			const triggerEl = e.detail.event.target as HTMLElement;
			const anchor = triggerEl.closest('a.response-anchor');
			if (!anchor) return;

			const parentPopover = triggerEl.closest('[data-tree-id]');
			let treeId: string;
			let level: number;

			// data-tree-id を持つ親ポップアップ内でクリックされた場合
			if (parentPopover) {
				// @ts-ignore
				treeId = parentPopover.dataset.treeId; // 既存のツリーIDを引き継ぐ
				level = parseInt(parentPopover.dataset.level || '0', 10) + 1;
			} else {
				treeId = `tree-click-${Date.now()}`; // クリック起点のツリーは独立させる
				level = 0;
			}
			showPopoverFor(anchor, treeId, level);
		}
	}

	// 被参照元一覧ポップアップを表示する
	async function showBacklinksPopover(targetResponseNumber: number, event: MouseEvent | KeyboardEvent, level: number) {

		// 自分より深い階層のポップアップを閉じる
		backlinksPopovers = backlinksPopovers.slice(0, level);
		// 同じレベルで同じポップアップを開こうとした場合は何もしない（既に開いている）
		if (backlinksPopovers[level]?.targetResponseNumber === targetResponseNumber) {
			return;
		}


		const sourceNumbers = backlinksMap.get(targetResponseNumber);
		if (!sourceNumbers || sourceNumbers.size === 0) return;

		const backlinkData = [...sourceNumbers]
			.map((num) => ({ response: findResponse(num), number: num }))
			.filter((item): item is { response: Post | Comment; number: number } => item.response !== null);

		if (backlinkData.length === 0) return;

		const newId = `backlinks-popover-${level}`;
		const newPopover: BacklinksPopoverState = {
			id: newId,
			level,
			targetResponseNumber,
			backlinks: backlinkData,
			position: { x: -9999, y: -9999 }
		};

		backlinksPopovers = [...backlinksPopovers, newPopover];
		await tick();

		// --- 位置計算 ---
		const popoverEl = document.getElementById(newId);
		const triggerEl = event.currentTarget as HTMLElement;

		if (!popoverEl || !triggerEl) {
			backlinksPopovers = backlinksPopovers.filter((p) => p.id !== newId);
			return;
		}

		// ポップアップの位置基準となる要素を探す。
		// クリックされた要素を含むレス全体(.response, .response-item, .popover, .backlinks-popover)を基準にする。
		const referenceElement = triggerEl.closest('.response, .response-item, .popover, .backlinks-popover') || triggerEl;
		const referenceRect = referenceElement.getBoundingClientRect();

		const popoverRect = popoverEl.getBoundingClientRect(); // この時点では幅・高さのみが有効
		const viewportWidth = window.innerWidth;
		const viewportHeight = window.innerHeight;
		const margin = 10;

		let x = referenceRect.left + 10;
		let y = referenceRect.bottom + 5;

		// 画面下にはみ出す場合は、トリガーの上に表示
		if (y + popoverRect.height > viewportHeight - margin) {
			y = referenceRect.top - popoverRect.height - 5;
		}
		if (x + popoverRect.width > viewportWidth - margin) { // 右にはみ出す場合
			x = viewportWidth - popoverRect.width - margin;
		}
		if (x < margin) {
			x = margin;
		}

		const popoverIndex = backlinksPopovers.findIndex((p) => p.id === newId);
		if (popoverIndex === -1) return;

		backlinksPopovers[popoverIndex].position = { x, y };
		backlinksPopovers = [...backlinksPopovers]; // Svelteに更新を通知
	}

	async function showIdPostsPopover(
		banType: 'User' | 'Ip' | 'Device',
		idPartValue: string,
		sourceId: number,
		event: MouseEvent | KeyboardEvent,
		canModerate: boolean
	) {

		// ポップアップを閉じる前に、トリガー要素の位置を取得します
		const triggerEl = event.currentTarget as HTMLElement;
		const referenceRect = triggerEl.getBoundingClientRect();

		// 既に同じIDのポップアップが開いている場合は何もしない
		if (idPostsPopover?.idPartValue === idPartValue && idPostsPopover?.banType === banType) {
			return;
		}
		idPostsPopover = null; // Close any existing one before opening a new one

		const newId = `id-posts-popover-${Date.now()}`;
		const newPopover: IdPostsPopoverState = {
			id: newId,
			banType,
			idPartValue,
			position: { x: -9999, y: -9999 },
			sourceId,
			canModerate
		};

		idPostsPopover = newPopover;
		await tick();

		const popoverEl = document.getElementById(newId);

		if (!popoverEl || !triggerEl) {
			idPostsPopover = null;
			return;
		}

		const popoverRect = popoverEl.getBoundingClientRect();
		const viewportWidth = window.innerWidth;
		const viewportHeight = window.innerHeight;
		const margin = 10; // 画面端からのマージン

		let y = referenceRect.bottom + 5;

		// 画面下にはみ出す場合は、トリガーの上に表示
		if (y + popoverRect.height > viewportHeight - margin) {
			y = referenceRect.top - popoverRect.height - 5;
		}
		// 上に表示しても画面上にはみ出す場合は、ビューポートの上端に固定
		if (y < margin) {
			y = margin;
		}

		let x = referenceRect.left;
		// 画面右にはみ出す場合は、位置を調整
		if (x + popoverRect.width > viewportWidth - margin) {
			x = viewportWidth - popoverRect.width - margin;
		}
		// 画面左にはみ出す場合は、位置を調整
		if (x < margin) {
			x = margin;
		}

		idPostsPopover.position = { x, y };
		idPostsPopover = { ...idPostsPopover };
	}

	function handleShowBacklinks(
		e: CustomEvent<{ responseNumber: number; event: MouseEvent | KeyboardEvent; level?: number }>
	) {
		if ($isMobile) {
			showBacklinksModal(e.detail.responseNumber);
		} else {
			const { responseNumber, event, level } = e.detail;
			showBacklinksPopover(responseNumber, event, level ?? 0);
		}
	}

	function handleShowIdPosts(
		e: CustomEvent<{
			banType: 'User' | 'Ip' | 'Device';
			idPartValue: string;
			event: MouseEvent | KeyboardEvent;
			sourceId: number;
			canModerate: boolean;
		}>
	) {
		if ($isMobile) {
			showIdPostsModal(e.detail.banType, e.detail.idPartValue, e.detail.sourceId);
		} else {
			showIdPostsPopover(
				e.detail.banType,
				e.detail.idPartValue,
				e.detail.sourceId,
				e.detail.event,
				e.detail.canModerate
			);
		}
	}

	// BAN Modal Logic
	async function openBanModal(type: 'post' | 'comment', id: number, banType?: 'User' | 'Ip' | 'Device') {
		const context = {
			boardId: data.board?.id,
			postId: data.post?.id
		};
		banTarget = { type, id, context, banType };
		identityDetails = null;
		isBanModalOpen = true;
		if ($page.data.user?.role === 'admin') {
			try {
				const query = type === 'post' ? `post_id=${id}` : `comment_id=${id}`;
				const response = await fetch(`/api/admin/identity-details?${query}`);
				if (response.ok) {
					identityDetails = await response.json();
				} else {
					const errorText = await response.text();
					console.error('個人情報の取得に失敗しました:', response.status, errorText);
				}
			} catch (error) {
				console.error('個人情報取得APIの呼び出し中にエラーが発生しました:', error);
			}
		}
	}

	function handleOpenBanModal(
		e: CustomEvent<{ sourceType: 'post' | 'comment'; sourceId: number; banType: 'User' | 'Ip' | 'Device' }>
	) {
		const { sourceType, sourceId, banType } = e.detail;
		// The BAN modal logic is simplified. It always receives a banType now.
		// The source (post/comment) is used to fetch identity details if needed.
		openBanModal(sourceType, sourceId, banType);
	}

	async function handleDeleteThread() {
		if (!data.post || !data.board) return;

		if (window.confirm('本当にこのスレッドを削除しますか？この操作は元に戻せません。')) {
			try {
				const response = await fetch(`/api/posts/${data.post.id}`, {
					method: 'DELETE',
					credentials: 'include'
				});

				if (response.ok) {
					alert('スレッドを削除しました。');
					await goto(`/boards/${data.board.id}`);
				} else {
					const errorData = await response.json().catch(() => ({ error: '削除に失敗しました。' }));
					alert(`削除に失敗しました: ${errorData.error || response.statusText}`);
				}
			} catch (error) {
				console.error('Error deleting thread:', error);
				alert('削除中にエラーが発生しました。');
			}
		}
	}

	function getLevelDisplay(entity: Post | Comment | null | undefined): string | null {
		// 作成時レベルが表示されない（閾値以上などの理由でnullにされた）場合は、何も表示しない
		if (entity?.level_at_creation == null) {
			return null;
		}

		const baseDisplay = `Lv.${entity.level_at_creation}`;

		// 現在レベルが表示可能で、かつ作成時レベルと異なる場合
		if (entity.level != null && entity.level !== entity.level_at_creation) {
			return `${baseDisplay}↝Lv.${entity.level}`;
		}

		// 現在レベルが閾値などの理由で隠されている場合
		if (entity.is_current_level_hidden) {
			return `${baseDisplay}↝?`;
		}

		// それ以外の場合（作成時と現在レベルが同じ、または現在レベルが存在しない）は、作成時のレベルのみを表示
		return baseDisplay;
	}

	function formatDateTime(isoString: string): string {
		if (!isoString) return '';
		const date = new Date(isoString);
		// APIからUTCで渡されるため、JSTで表示するようにタイムゾーンを指定
		return date.toLocaleString('ja-JP', { timeZone: 'Asia/Tokyo' });
	}

	// 日付を相対時間で整形するヘルパー関数
	function formatRelativeTime(isoString: string): string {
		if (!isoString) return '';
		// 常に現在時刻（JST）との差を計算
		const now = new Date();
		// APIから来る時刻はUTCなので、JSTに変換してから差を計算
		const pastJST = new Date(new Date(isoString).toLocaleString('en-US', { timeZone: 'Asia/Tokyo' }));

		const diffInSeconds = Math.floor((now.getTime() - pastJST.getTime()) / 1000);

		if (diffInSeconds < 60) {
			return 'たった今';
		}
		const minutes = Math.floor(diffInSeconds / 60);
		if (minutes < 60) {
			return `${minutes}分前`;
		}
		const hours = Math.floor(minutes / 60);
		if (hours < 24) {
			return `${hours}時間前`;
		}
		const days = Math.floor(hours / 24);
		return `${days}日前`;
	}

	// 勢いを整形するヘルパー関数
	function formatMomentum(momentum: number | null | undefined): string {
		return (momentum ?? 0).toFixed(2);
	}

	// スレッドが1000レスに達しているかどうかの判定 (元の投稿 + コメント数)
	// --- Reactive Statements ---
	$: postLevelDisplay = getLevelDisplay(data.post);

	// モーダル表示中は背景のスクロールを禁止する
	$: {
		if (browser) {
			if (mobileModalStack.length > 0) {
				document.body.style.overflow = 'hidden';
			} else {
				document.body.style.overflow = '';
			}
		}
	}

	// 各レスへの参照元レス番号を計算する
	$: backlinksMap = (() => {
		if (!data.post || !data.comments) {
			return new Map<number, Set<number>>();
		}

		const newBacklinks = new Map<number, Set<number>>();
		const allResponses = [data.post, ...data.comments];

		allResponses.forEach((response, index) => {
			if (!response) return;
			const sourceResponseNumber = index + 1;
			const rawBody = response.body || '';
			const decodedBody = decodeHtmlEntities(rawBody);

			const uniqueTargetsInBody = new Set<number>();
			const anchorRegex = />>(\d+)/g;
			let match;
			while ((match = anchorRegex.exec(decodedBody)) !== null) {
				uniqueTargetsInBody.add(parseInt(match[1], 10));
			}
			uniqueTargetsInBody.forEach((targetResponseNumber) => {
				if (!newBacklinks.has(targetResponseNumber)) {
					newBacklinks.set(targetResponseNumber, new Set());
				}
				newBacklinks.get(targetResponseNumber)?.add(sourceResponseNumber);
			});
		});
		return newBacklinks;
	})();

	// backlinksMap から被参照数を計算
	$: backlinkCounts = new Map([...backlinksMap.entries()].map(([k, v]) => [k, v.size]));

	$: isThreadFull = (data.comments?.length || 0) + 1 >= 1000;

	// フローティングボタンを表示するかどうか
	$: showFloatingButton = data.post && !data.post.archived_at && !isThreadFull;

	// ソートされたスレッド一覧
	$: sortedPosts = data.posts
		? [...data.posts].sort((a, b) => {
				const [key, direction] = sortOrder.split('_');
				const dir = direction === 'asc' ? 1 : -1;

				let valA, valB;

				switch (key) {
					case 'momentum':
						valA = a.momentum ?? 0;
						valB = b.momentum ?? 0;
						break;
					case 'responses':
						valA = a.response_count ?? 0;
						valB = b.response_count ?? 0;
						break;
					case 'last_activity':
						valA = new Date(a.last_activity_at).getTime();
						valB = new Date(b.last_activity_at).getTime();
						break;
					case 'created_at':
						valA = new Date(a.created_at).getTime();
						valB = new Date(b.created_at).getTime();
						break;
					default:
						return 0;
				}

				return (valA < valB ? -1 : valA > valB ? 1 : 0) * dir;
			})
		: [];

	// レスジャンプナビゲーションのリンクを生成
	$: jumpLinks = (() => {
		const total = (data.comments?.length || 0) + 1;
		// レスが1つ以下の場合はナビゲーション不要
		if (total <= 1) return [];

		const links = [{ label: '1', number: 1 }];
		for (let i = 100; i <= 1000; i += 100) {
			if (total >= i) {
				links.push({ label: `${i}`, number: i });
			}
		}
		links.push({ label: '最新', number: total });
		return links;
	})();

	// --- Lifecycle Hooks ---
	onMount(() => {
		if (typeof window !== 'undefined') {
			if (window.matchMedia('(max-width: 768px)').matches) {
				isPanelOpen = false;
			}
			// Escapeキーでのポップアップクローズは維持
			window.addEventListener('keydown', handleKeydown);
		}
	});

	onDestroy(() => {
		if (typeof window !== 'undefined') {
			// keydownリスナーのみ削除
			window.removeEventListener('keydown', handleKeydown);
			closeCommentFormPanel();
		}
	});

	function handleMainContentClick(event: MouseEvent) {
		if (!$isMobile) return;
		const anchor = (event.target as HTMLElement).closest('a.response-anchor');
		if (anchor) {
			event.preventDefault();
			const href = anchor.getAttribute('href');
			const match = href?.match(/#res-(\d+)/);
			if (match?.[1]) {
				showResponseModal(parseInt(match[1], 10));
			}
		}
	}

	/**
	 * 文字列内のURLを検出し、クリック可能な<a>タグに変換します。
	 * @param text 処理対象のテキスト（HTMLを含む可能性がある）
	 * @returns リンクが変換されたHTML文字列
	 */
	function linkifyUrls(text: string): string {
		if (!text) return '';
		const urlRegex = /(https?:\/\/[^\s<>"']+)/g;
		return text.replace(urlRegex, (url) => {
			return `<a href="${url}" target="_blank" rel="noopener noreferrer" class="external-link">${url}</a>`;
		});
	}

	/**
	 * サイドパネル上でのマウスホイールイベントを処理し、意図しない本体のスクロールを防ぎます。
	 * @param event マウスホイールイベント
	 */
	function handlePanelWheel(event: WheelEvent) {
		event.stopPropagation(); // イベントの伝播を停止し、ページ本体がスクロールするのを完全に防ぐ
	}
</script>

{#if data.error}
	<p class="error">{data.error}</p>
{:else if data.post && data.board && data.posts}
	<div
		class="page-layout"
		class:panel-closed={!isPanelOpen}
		class:resizing={isResizing}
		on:mouseover={handleGlobalMouseOver}
	>
		<div class="sticky-container">
			<aside
				class="side-panel"
				style:width={isPanelOpen ? `${panelWidth}px` : '0'}
				on:wheel|self={handlePanelWheel}
			>
				<!-- パネル内のコンテンツ全体をラップするコンテナを追加 -->
				<div class="side-panel-content">
					<div class="description-container">
						<p class="board-description">{data.board.description}</p>
					</div>

					<div class="sort-container">
						<label for="sort-order-panel">表示順:</label>
						<select id="sort-order-panel" bind:value={sortOrder}>
							{#each sortOptions as option}
								<option value={option.value}>{option.label}</option>
							{/each}
						</select>
					</div>

					<div class="post-list-container">
						<ul class="post-list">
							{#each sortedPosts as p (p.id)}
								<li class:current={p.id === data.post.id}>
									<a href="/posts/{p.id}" class="post-link-container" title={decodeHtmlEntities(p.title)}>
										<div class="post-info">
											<div class="post-title-wrapper">
												<span class="post-title">{decodeHtmlEntities(p.title)}</span>
											</div>
											<div class="post-meta">
												<span class="post-responses">レス: {p.response_count ?? 0}</span>
												<span class="post-momentum">勢い: {formatMomentum(p.momentum)}</span>
												<small class="post-timestamp" title={formatDateTime(p.last_activity_at)}>
													更新: {formatRelativeTime(p.last_activity_at)}
												</small>
												<small class="post-timestamp">作成: {formatDateTime(p.created_at)}</small>
											</div>
										</div>
									</a>
								</li>
							{/each}
						</ul>
					</div>
				</div>
			</aside>

			<!-- パネル幅を調整するためのリサイザー -->
			<div class="resizer" on:mousedown={handleMouseDown} />
		</div>

		<main
			class="main-content"
			style:margin-left={isPanelOpen ? `calc(${panelWidth}px + 6px + 3rem)` : '1.5rem'}
			on:click={handleMainContentClick}
		>
			<div class="thread-container">
				{#if jumpLinks.length > 0}
					<div class="jump-nav">
						<span>|</span>{#each jumpLinks as link}<a href="#res-{link.number}">{link.label}</a><span>|</span>{/each}
					</div>
				{/if}

				<section class="responses">
					<article class="response" id="res-1">
						<div class="response-header">
							{#if backlinkCounts.get(1)}
								<span
									class="response-number clickable-backlinks"
									role="button"
									tabindex="0"
									on:click|stopPropagation={(e) => {
										$isMobile
											? showBacklinksModal(1)
											: handleShowBacklinks({ detail: { responseNumber: 1, event: e, level: 0 } });
									}}
									on:keydown|stopPropagation={(e) => {
										if (e.key === 'Enter') $isMobile ? showBacklinksModal(1) : showBacklinksPopover(1, e, 0);
									}}
								>
									1: ({backlinkCounts.get(1)})
								</span>
							{:else}
								<span class="response-number">1:</span>
							{/if}
							<span class="response-author">{data.post.author_name || '鶏民'}</span>
							<span class="response-timestamp">{formatDateTime(data.post.created_at)}</span>
							{#if postLevelDisplay}
								<span>
									{postLevelDisplay}
								</span>
							{/if}
							{#if isAdmin}
								<button class="delete-thread-button" on:click={handleDeleteThread} title="このスレッドを削除します">
									スレッド削除
								</button>
							{/if}
							{#if data.post.display_user_id}
								<span class="response-id">
									ID:
									{#each data.post.display_user_id.split('-') as part, i}
										{@const type = i === 0 ? 'User' : i === 1 ? 'Ip' : 'Device'}
										<span
											class="id-part"
											class:bannable={data.post.can_moderate && type !== 'Device'}
											on:click|stopPropagation={(e) =>
												handleShowIdPosts({
													detail: {
														banType: type,
														idPartValue: part,
														event: e,
														sourceId: data.post.id,
														canModerate: data.post.can_moderate ?? false
													}
												})}
											on:keydown|stopPropagation={(e) =>
												e.key === 'Enter' &&
												handleShowIdPosts({
													detail: {
														banType: type,
														idPartValue: part,
														event: e,
														sourceId: data.post.id,
														canModerate: data.post.can_moderate ?? false
													}
												})
											}
											role="button"
											tabindex="0"
										>{part}</span>{#if i < 2}-{/if}
									{/each}
								</span>
							{/if}
						</div>
						<div class="response-body">
							{@html linkifyUrls(fixDoubleEscaping(data.post.body))}
						</div>
					</article>

					{#each data.comments as comment, i}
						{@const commentLevelDisplay = getLevelDisplay(comment)}
						{@const responseNumber = i + 2}
						<article class="response" id="res-{i + 2}">
							<div class="response-header">
								{#if backlinkCounts.get(responseNumber)}
									<span
										class="response-number clickable-backlinks"
										role="button"
										tabindex="0"
									on:click|stopPropagation={(e) => {
										$isMobile
											? showBacklinksModal(responseNumber)
											: handleShowBacklinks({ detail: { responseNumber, event: e, level: 0 } });
									}}
									on:keydown|stopPropagation={(e) => {
										if (e.key === 'Enter') $isMobile ? showBacklinksModal(responseNumber) : handleShowBacklinks({ detail: { responseNumber, event: e, level: 0 } });
									}}
									>
										{responseNumber}: ({backlinkCounts.get(responseNumber)})
									</span>
								{:else}
									<span class="response-number">{responseNumber}:</span>
								{/if}
								<span class="response-author">{comment.author_name || '鶏民'}</span>
								<span class="response-timestamp">{formatDateTime(comment.created_at)}</span>
								{#if commentLevelDisplay}
									<span>
										{commentLevelDisplay}
									</span>
								{/if}
								{#if comment.display_user_id}
									<span class="response-id">
										ID:
										{#each comment.display_user_id.split('-') as part, i}
											{@const type = i === 0 ? 'User' : i === 1 ? 'Ip' : 'Device'}
											<span
												class="id-part"
												class:bannable={comment.can_moderate && type !== 'Device'}
												on:click|stopPropagation={(e) =>
													handleShowIdPosts({
														detail: {
															banType: type,
															idPartValue: part,
															event: e,
															sourceId: comment.id,
															canModerate: comment.can_moderate ?? false
														}
													})}
												on:keydown|stopPropagation={(e) =>
													e.key === 'Enter' &&
													handleShowIdPosts({
														detail: {
															banType: type,
															idPartValue: part,
															event: e,
															sourceId: comment.id,
															canModerate: comment.can_moderate ?? false
														}
													})
												}
												role="button"
												tabindex="0"
											>{part}</span>{#if i < 2}-{/if}
										{/each}
									</span>
								{/if}
							</div>
							<div class="response-body">
								{@html linkifyUrls(fixDoubleEscaping(comment.body))}
							</div>
						</article>
					{/each}
				</section>

				{#if jumpLinks.length > 0}
					<div class="jump-nav">
						<span>|</span>{#each jumpLinks as link}<a href="#res-{link.number}">{link.label}</a><span>|</span>{/each}
					</div>
				{/if}

				<hr class="form-separator" />

				{#if data.post.archived_at}
					<div class="archived-notice">
						<p>このスレッドは過去ログ倉庫に格納されているため、新しいコメントを書き込むことはできません。</p>
						<a href="/archive">過去ログ倉庫に戻る</a>
					</div>
				{:else if isThreadFull}
					<p class="thread-full-message">
						このスレッドは1000レスに達しました。新しいレスは投稿できません。
					</p>
				{/if} <!-- フォーム表示条件の終了 -->
			</div> <!-- .thread-container -->
		</main> <!-- .main-content -->

		<!-- パネル開閉ボタン (デスクトップ/モバイルで位置が変わる) -->
		<button
			class="toggle-panel-button"
			on:click={() => (isPanelOpen = !isPanelOpen)}
			title="サイドパネルを開閉"
			style:left={isPanelOpen ? `calc(1.5rem + ${panelWidth}px - 20px)` : `calc(1.5rem - 20px)`}
		>
			<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
				<polygon points="18 4, 6 12, 18 20" />
			</svg>
		</button>

	</div> <!-- .page-layout -->
{/if}

<!-- BANモーダルコンポーネントを配置 -->
{#if isBanModalOpen}
	<div class="ban-modal-overlay" on:click={() => (isBanModalOpen = false)}>
		<div class="ban-modal-wrapper" on:click|stopPropagation>
			<BanModal
				bind:isOpen={isBanModalOpen}
				{banTarget}
				{identityDetails}
				canModerateBoard={data.board?.can_moderate ?? false}
			/>
		</div>
	</div>
{/if}

{#if $isMobile}
	<!-- モバイル用モーダルスタック -->
	{#if mobileModalStack.length > 0}
		<div class="mobile-modal-overlay" on:click={closeTopMobileModal}>
			{#each mobileModalStack as modal, i (modal.id)}
				<!-- 各モーダルのコンテンツを配置するラッパー。これが白い箱になる -->
				<div class="mobile-modal-wrapper" on:click|stopPropagation style:z-index={i * 2}>
					{#if modal.type === 'response'}
						<ResponsePopover
							{...modal.props}
							asModalContent={true}
							on:showbacklinks={handleShowBacklinks}
							on:openbanmodal={handleOpenBanModal}
							on:showidposts={handleShowIdPosts}
							on:showresponse={handleShowResponse}
							on:close={closeTopMobileModal}
						/>
					{:else if modal.type === 'backlinks'}
						<BacklinksPopover
							{...modal.props}
							asModalContent={true}
							on:showbacklinks={handleShowBacklinks}
							on:openbanmodal={handleOpenBanModal}
							on:showidposts={handleShowIdPosts}
							on:showresponse={handleShowResponse}
							on:close={closeTopMobileModal}
						/>
					{:else if modal.type === 'id-posts'}
						<IdPostsPopover
							{...modal.props}
							asModalContent={true}
							backlinkCounts={backlinkCounts}
							on:openbanmodal={handleOpenBanModal}
							on:showidposts={handleShowIdPosts}
							on:showresponse={handleShowResponse}
							on:showbacklinks={handleShowBacklinks}
							on:close={closeTopMobileModal}
						/>
					{/if}
				</div>

				<!-- 一番手前以外のモーダルを暗くし、操作不能にするためのオーバーレイ -->
				{#if i < mobileModalStack.length - 1}
					<div class="modal-dimmer" style:z-index={i * 2 + 1} />
				{/if}
			{/each}
		</div>
	{/if}
{:else}
	<!-- デスクトップ用ポップアップ -->
	{#each [...popoverTrees.values()].flat() as popover (popover.id)}
		{@const treeId = popover.id.split('-').slice(1, -1).join('-')}
			<ResponsePopover
				data-tree-id={treeId}
				id={popover.id}
				response={popover.response}
				position={popover.position}
				responseNumber={popover.responseNumber}
				dataLevel={popover.level}
				backlinkCount={backlinkCounts.get(popover.responseNumber)}
				on:mouseenter={() => handlePopoverMouseEnter({ detail: { treeId } })}
				on:mouseleave={() => handlePopoverMouseLeave({ detail: { treeId } })}
				on:close={() => { const tree = popoverTrees.get(treeId); if (tree) { popoverTrees.set(treeId, tree.filter(p => p.id !== popover.id)); popoverTrees = new Map(popoverTrees); } }}
				on:openbanmodal={handleOpenBanModal}
				on:showresponse={handleShowResponse}
				on:showbacklinks={handleShowBacklinks}
				on:showidposts={handleShowIdPosts}
			/>
	{/each}
	{#each backlinksPopovers as popover (popover.id)}
		<BacklinksPopover
			id={popover.id}
			level={popover.level}
			backlinks={popover.backlinks}
			position={popover.position}
			targetResponseNumber={popover.targetResponseNumber}
			backlinkCounts={backlinkCounts}
			on:close={() => backlinksPopovers = backlinksPopovers.filter(p => p.id !== popover.id)}
			on:openbanmodal={handleOpenBanModal}
			on:showbacklinks={handleShowBacklinks}
			on:showresponse={handleShowResponse}
			on:showidposts={handleShowIdPosts}
		/>
	{/each}
	{#if idPostsPopover}
		<IdPostsPopover
			id={idPostsPopover.id}
			banType={idPostsPopover.banType}
			idPartValue={idPostsPopover.idPartValue}
			posts={[data.post, ...data.comments]}
			canModerate={idPostsPopover.canModerate}
			isAdmin={isAdmin}
			sourceId={idPostsPopover.sourceId}
			position={idPostsPopover.position}
			backlinkCounts={backlinkCounts}
			on:close={() => (idPostsPopover = null)}
			on:openbanmodal={handleOpenBanModal}
			on:showresponse={handleShowResponse}
			on:showidposts={handleShowIdPosts}
			on:showbacklinks={handleShowBacklinks}
		/>
	{/if}
{/if}

{#if data.post}
	<CommentFormPanel postId={data.post.id} />
{/if}

{#if showFloatingButton}
	<button
		class="floating-action-button"
		title="新しいレスを投稿"
		on:click|preventDefault={openCommentFormPanel}
		transition:fly={{ y: 100, duration: 300 }}
	>
		<!-- ペンアイコン -->
		<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<path d="M12 20h9" />
			<path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
		</svg>
	</button>
{/if}

<style>
	/* 2カラムレイアウト */
	.page-layout {
		/* The layout is now controlled by the fixed panel and the main content's margin */
	}

	.page-layout.resizing .main-content {
		/* リサイズ中はアニメーションを無効化し、直接的な反応にする */
		transition: none;
	}
	.page-layout.resizing .toggle-panel-button {
		/* リサイズ中はアニメーションを無効化し、直接的な反応にする */
		transition: none;
	}

	.sticky-container {
		position: fixed;
		top: var(--header-height, 4rem); /* ヘッダーの真下に配置 */
		bottom: 0; /* 画面の最下部まで伸ばす */
		left: 1.5rem; /* 画面左端からの水平オフセット */
		display: flex;
		z-index: 20; /* Ensure it's above other content */
	}

	.side-panel {
		height: 100%;
		transition:
			width 0.3s ease, /* パネル開閉時のアニメーション */
			padding 0.3s ease, /* パネル開閉時のアニメーション */
			opacity 0.2s ease 0.1s; /* スクロール復元後のフェードインアニメーション */
		overflow-y: auto; /* パネル全体をスクロール可能にする */
		display: flex; /* 子要素がflexで高さを埋めるように */
		flex-direction: column;
		padding: 1rem 1rem 0 0; /* 上1rem, 右1.5rem の余白 */
		background-color: #f0f2f5; /* ページの色(フォーム背景色)に合わせ、背後が透けないようにする */
	}

	.side-panel-content {
		width: 100%;
		display: flex;
		flex-direction: column;
	}

	/* パネル内のコンテンツの表示・非表示を制御 */
	.side-panel-content {
		/* 開く時: パネルが少し開いてからコンテンツを表示 */
		transition: opacity 0.15s ease 0.15s;
	}

	.page-layout.panel-closed .side-panel-content {
		/* 閉じる時: 即座にコンテンツを非表示 */
		opacity: 0;
		transition: opacity 0.15s ease;
		pointer-events: none; /* 非表示のコンテンツがクリックされるのを防ぐ */
	}

	/* パネルが閉じているときのスタイル */
	.page-layout.panel-closed .side-panel {
		padding: 1rem 0 0 0; /* 閉じる時は上余白のみ維持 */
		opacity: 0;
	}

	.resizer {
		width: 6px;
		cursor: col-resize;
		background-color: #f0f2f5; /* リサイズ領域を明確にするための背景色 */
		border-right: 1px solid #ccc; /* 境界線を明確にするための線 */
		transition: background-color 0.2s ease;
		height: 100%;
		z-index: 5;
	}
	.resizer:hover,
	.page-layout.resizing .resizer {
		background-color: #007bff; /* ホバー時またはリサイズ中に色を付ける */
	}
	.page-layout.panel-closed .resizer {
		display: none;
	}

	.main-content {
		position: relative;
		/* パネル開閉時のアニメーションをスムーズにするためのtransition */
		transition: margin-left 0.3s ease;
	}

	.toggle-panel-button {
		position: fixed;
		top: calc(var(--header-height, 4rem) + 16px);
		z-index: 25; /* パネル(z-index: 20)より手前に表示 */
		background: #fff;
		border: 1px solid #ccc;
		border-radius: 4px;
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		color: #666;
		transition: background-color 0.2s ease, color 0.2s ease, left 0.3s ease;
	}
	.toggle-panel-button:hover {
		background-color: #e9ecef;
	}
	.toggle-panel-button svg {
		transition: transform 0.3s ease;
	}
	.page-layout.panel-closed .toggle-panel-button svg {
		transform: rotate(180deg);
	}

	.overlay {
		display: none; /* デスクトップでは非表示 */
	}

	/* --- スマートフォン向けのスタイル (レスポンシブ対応) --- */
	@media (max-width: 768px) {
		.page-layout {
			display: block; /* レイアウトを単純なブロック要素に戻す */
		}

		.main-content {
			margin-left: 0 !important; /* マージンをリセット */
		}

		/* スマートフォンではサイドパネルと関連要素を非表示にする */
		.sticky-container,
		.resizer,
		.toggle-panel-button {
			display: none;
		}

		.thread-container {
			/* モバイルでは最大幅と左右マージンをリセットし、横幅いっぱいに表示 */
			max-width: none;
			margin: 0;
		}

		.floating-action-button {
			/* モバイルでは少し小さくし、位置を調整 */
			width: 48px;
			height: 48px;
			bottom: 1.5rem;
			right: 1.5rem;
		}

		.floating-action-button svg {
			width: 20px;
			height: 20px;
		}
	}

	.modal-dimmer {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background-color: rgba(30, 30, 30, 0.5); /* 背景のモーダルを暗くする */
	}

	.mobile-modal-overlay {
		position: fixed;
		left: 0;
		bottom: 0;
		width: 100%;		
		background-color: rgba(0, 0, 0, 0.6);
		/* ヘッダーを覆わないように、オーバーレイの開始位置をヘッダーの高さ分下げる */
		height: calc(100% - var(--header-height, 4rem));
		z-index: 2000; /* 他の要素より手前に表示 */
	}

	.mobile-modal-wrapper {
		position: absolute;
		top: 0;
		left: 0;		
		width: 100%;
		/* 画面下に常にタップ可能な領域を確保するため、高さを調整 */
		max-height: calc(100% - 4rem);
		background-color: #ffffff;
		overflow-y: auto;
		-webkit-overflow-scrolling: touch; /* iOSで慣性スクロールを有効にする */
		overscroll-behavior-y: contain; /* モーダル端での背景スクロールを防止 */
	}

	/* サイドパネル内のスタイル */
	.description-container {
		/* 板名とIDを削除したため、flexbox関連のスタイルは不要になりました */
		margin-bottom: 1rem;
	}
	.board-description {
		font-size: 0.9rem;
		color: #666;
		margin: 0; /* 親コンテナでマージンを管理するためリセット */
		min-width: 0; /* 長い説明文がはみ出さないように */
		word-break: break-all; /* URLなど長い文字列がはみ出すのを防ぐ */
	}

	.side-panel .sort-container {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}
	.side-panel .sort-container label {
		font-weight: 500;
		font-size: 0.9em;
		flex-shrink: 0;
	}
	.side-panel .sort-container select {
		width: 100%;
		padding: 0.5rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		font-size: 0.9em;
	}

	.post-list-container {
		/* スクロールした際に最後の要素が見切れないように下部に余白を追加 */
		/* 利用可能な残りの高さをすべて使い、コンテンツが溢れたらスクロールする */
		flex: 1;
		overflow-y: auto;
		padding-bottom: 2rem;
	}
	.post-list {
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.post-list li + li {
		border-top: 1px solid #eee; /* 区切り線 */
	}

	.post-list li a.post-link-container {
		display: block;
		text-decoration: none;
		color: inherit;
		padding: 0.75rem 0.5rem;
		border-radius: 4px;
		transition: background-color 0.2s ease;
	}

	.post-list li:hover a.post-link-container {
		background-color: #f0f0f0;
	}

	.post-list li.current a.post-link-container {
		background-color: #e7f3ff;
	}

	.post-info {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		width: 100%;
	}

	.post-title-wrapper {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
	}

	.post-title {
		white-space: normal; /* テキストの折り返しを許可 */
		word-break: break-all; /* 長い単語がはみ出すのを防ぐ */
		font-weight: 500;
	}

	.post-list li.current .post-title {
		font-weight: bold;
		color: #0056b3;
	}

	.post-meta {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.2rem 0.6rem;
		color: #555;
		font-size: 0.8em;
	}
	.post-responses,
	.post-momentum {
		font-weight: bold;
		white-space: nowrap;
	}
	.post-timestamp {
		white-space: nowrap;
	}
	.archived-notice {
		margin-top: 2rem;
		padding: 1rem;
		background-color: #f0f0f0;
		border: 1px solid #ddd;
		border-radius: 8px;
		text-align: center;
	}

	.thread-full-message {
		color: #d32f2f; /* 赤色 */
		font-weight: bold;
		text-align: center;
		padding: 1rem;
		border: 1px solid #d32f2f;
		border-radius: 8px;
		background-color: #ffebee; /* 薄い赤色の背景 */
	}
	.form-separator {
		margin: 2rem 0;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		margin-top: 1rem;
		padding: 1.5rem;
		border: 1px solid #ddd;
		border-radius: 8px;
		background-color: #f9f9f9;
	}
	form div {
		display: flex;
		flex-direction: column;
	}
	form label {
		margin-bottom: 0.5rem;
		font-weight: 500;
	}
	form input,
	form textarea {
		padding: 0.75rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		font-size: 1rem;
	}
	form button {
		padding: 0.75rem 1.5rem;
		border-radius: 4px;
		border: none;
		background-color: #2d8cff;
		color: white;
		font-size: 1rem; /* 継承されるはずだが明示的に指定 */
		cursor: pointer;
		transition: background-color 0.2s;
	}
	form button:hover {
		background-color: #0070e0;
	}
	form button:disabled {
		background-color: #ccc;
		cursor: not-allowed;
	}
	.error-message {
		color: red;
		margin-top: 0.5rem;
		white-space: pre-wrap; /* 改行を反映 */
	}
	.field-error {
		font-size: 0.9em;
		margin-top: 0.25rem;
	}

	.input-wrapper {
		position: relative;
	}

	.char-counter {
		position: absolute;
		bottom: 8px;
		right: 8px;
		font-size: 0.8em;
		color: #666;
		background-color: rgba(249, 249, 249, 0.8); /* フォーム背景色に合わせる */
		padding: 1px 4px;
		border-radius: 3px;
		pointer-events: none; /* カウンターがテキストエリアの操作を妨げないように */
	}
	form textarea + .char-counter {
		bottom: 12px;
	}

	/* 既存のスタイル */

	.thread-container {
		max-width: 800px;
		margin: 0rem auto 2rem;
	}
	.response {
		margin-bottom: 1.5rem;
		/* クリックで移動した際に、固定ヘッダーに隠れないように上部にマージンを設定 */
		/* var(--header-height) はグローバルレイアウトで定義されている想定 */
		scroll-margin-top: calc(var(--header-height, 4rem) + 1rem);	
	}
	.response-author {
		font-weight: bold;
		color: #007bff;
	}
	.response-header {
		margin-bottom: 0.5rem;
		color: #666;
		font-size: 0.9em;
		display: flex;
		flex-wrap: wrap;
		gap: 0.25em 0.5em; /* 半角スペースに調整 */
	}
	.response-header .response-number {
		font-weight: bold;
	}
	.response-number.clickable-backlinks {
		cursor: pointer;
	}
	.response-number.clickable-backlinks:hover {
		text-decoration: underline;
	}
	.response-body {
		line-height: 1.5;
		/* 長い文字列がレイアウトを崩さないように、強制的に改行する */
		word-break: break-all;
	}
	/* 本文内の要素のデフォルトマージンをリセットし、要素間のスペースを制御する */
	.response-body > :global(*) {
		margin: 0;
	}
	.response-body > :global(* + *) {
		margin-top: 1em;
	}

	/* 外部リンク用のスタイル */
	.response-body :global(a.external-link) {
		color: #008000; /* 緑色など、他のリンクと区別できる色 */
		text-decoration: underline;
	}
	.response-body :global(a.external-link:hover) {
		color: #006400; /* ホバー時の色 */
	}
	.id-part.bannable {
		cursor: pointer;
		color: #007bff;
	}
	.id-part.bannable:hover {
		color: #0056b3;
	}
	.ban-modal-overlay {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background-color: rgba(0, 0, 0, 0.6);
		display: flex;
		justify-content: center;
		align-items: center;
		z-index: 9999; /* 全てのUI要素の最前面に表示 */
	}
	.ban-modal-wrapper {
		/* BanModalコンポーネント自体のクリックでオーバーレイが閉じないようにする */
	}
	.error {
		color: red;
		font-weight: bold;
	}

	.delete-thread-button {
		margin-left: auto; /* 右寄せにする */
		background-color: #dc3545;
		color: white;
		border: none;
		padding: 0.2rem 0.6rem;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.8em;
		font-weight: bold;
		transition: background-color 0.2s;
	}

	.delete-thread-button:hover {
		background-color: #c82333;
	}

	.floating-action-button {
		position: fixed;
		bottom: 2rem;
		right: 2rem;
		width: 56px;
		height: 56px;
		background-color: #2d8cff;
		color: white;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2); /* 影 */
		z-index: 1005; /* ヘッダーよりは下、コンテンツよりは上 */
		text-decoration: none;
		transition: background-color 0.2s ease, transform 0.2s ease;
	}

	.floating-action-button:hover {
		background-color: #0070e0;
		transform: translateY(-2px);
	}
	
	.jump-nav {
		margin: 1rem 0;
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		background-color: #f0f2f5;
		padding: 0.5rem;
		border-radius: 4px;
	}
	/* 上部のナビゲーションバーの上の余白をなくす */
	.thread-container > .jump-nav:first-child {
		margin-top: 0;
	}

	.jump-nav a {
		margin: 0 0.25rem;
		text-decoration: none;
		color: #007bff;
	}
	.jump-nav a:hover {
		text-decoration: underline;
	}
	@media (max-width: 768px) {
		.floating-action-button {
			bottom: 1.5rem;
			right: 1.5rem;
		}
	}
</style>
