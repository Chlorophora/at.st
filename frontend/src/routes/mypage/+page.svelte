<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { PageData, ActionData } from './$types';
	import { PUBLIC_TURNSTILE_SITE_KEY } from '$env/static/public';
	import { invalidateAll } from '$app/navigation';
	import RegenerateToken from '$lib/components/RegenerateToken.svelte';
	import { browser } from '$app/environment';
	import { getRawFingerprintData } from '$lib/utils/fingerprint';
	import { enhance } from '$app/forms';

	export let data: PageData;
	export let form: ActionData;

	let turnstileContainer: HTMLDivElement; // Turnstileウィジェットをレンダリングするコンテナ
	let widgetId: string | null = null; // TurnstileウィジェットのID
	let isLoading = false;
	let clientMessage = ''; // データ収集時など、クライアントサイドで発生したエラーメッセージ
	let turnstileToken = ''; // Turnstileから受け取ったトークンを保持します

	// Countdown timer state
	let remainingSeconds: number | null = data.levelUpStatus?.lock_expires_in_seconds ?? null;
	let countdownDisplay = '';
	let timerInterval: any;

	// グローバルスコープに関数を登録する必要があるため、型定義を拡張
	declare global {
		interface Window {
			turnstile: any;
		}
	}

	// 秒数を HH:MM:SS 形式の文字列にフォーマットします
	function formatDuration(seconds: number) {
		if (seconds <= 0) return '00:00:00';
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = seconds % 60;
		return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
	}

	// カウントダウンタイマーを開始します
	function startCountdown() {
		if (timerInterval) clearInterval(timerInterval);
		if (remainingSeconds !== null && remainingSeconds > 0) {
			timerInterval = setInterval(() => {
				remainingSeconds!--;
				countdownDisplay = formatDuration(remainingSeconds!);
				if (remainingSeconds! <= 0) {
					clearInterval(timerInterval);
					// 時間が来たら、サーバーから最新の状態を取得するためにページデータを再読み込みします
					invalidateAll();
				}
			}, 1000);
			countdownDisplay = formatDuration(remainingSeconds);
		}
	}

	// ページ読み込み時に Turnstile コンポーネントをインポート
	onMount(() => {
		// Turnstileのレンダリング関数
		const renderTurnstile = () => {
			if (!turnstileContainer || widgetId) return; // コンテナがない、または既にレンダリング済みの場合は何もしない

			try {
				widgetId = window.turnstile.render(turnstileContainer, {
					sitekey: PUBLIC_TURNSTILE_SITE_KEY,
					callback: function (token: string) {
						console.log('Turnstile: Success! Token:', token);
						turnstileToken = token;
					},
					'expired-callback': () => {
						console.log('Turnstile: Expired');
						turnstileToken = '';
					},
					'error-callback': () => {
						console.error('Turnstile: Error');
						turnstileToken = '';
						clientMessage = 'ボット検証ウィジェットの読み込みに失敗しました。';
					}
				});
			} catch (error) {
				console.error('Failed to render Turnstile widget:', error);
				clientMessage = 'ボット検証ウィジェットの表示に失敗しました。';
			}
		};

		// ユーザーがロックされている場合は、カウントダウンを開始します
		if (data.levelUpStatus?.is_locked) {
			startCountdown();
		}

		// `window.turnstile` が利用可能になるのを待ってからレンダリングを実行
		const interval = setInterval(() => {
			if (window.turnstile) {
				clearInterval(interval);
				renderTurnstile();
			}
		}, 100);

		// onMountからクリーンアップ関数を返す
		return () => {
			clearInterval(interval);
		};
	});

	onDestroy(() => {
		// コンポーネントが破棄される際に、タイマーをクリーンアップします
		if (timerInterval) {
			clearInterval(timerInterval);
		}

		// Turnstileウィジェットもクリーンアップします
		if (widgetId && window.turnstile) {
			window.turnstile.remove(widgetId);
		}
	});
</script>

<svelte:head>
	<title>マイページ</title>
</svelte:head>

<h1>マイページ</h1>

<div class="user-status">
	{#if data.user}
		<p>
			ようこそ、<strong>{data.user.email}</strong>さん！ (Role: {data.user.role})
		</p>
	{:else}
		<p>
			<a href="/auth/register">認証はこちら</a>
		</p>
	{/if}
</div>

{#if data.user}
	<p>現在のレベル Lv:{data.user.level}</p>

	<div class="level-up-section">
		<h2>レベルアップ</h2>

		<!-- サーバーからのメッセージを表示 -->
		{#if form?.message}
			<p class:success={form.success} class:error={!form.success}>{form.message}</p>
		{/if}

		<!-- クライアントサイドでのエラーメッセージ -->
		{#if clientMessage}
			<p class="error">{clientMessage}</p>
		{/if}

		{#if data.levelUpStatus}
			<!-- Case 1: レート制限中、またはその他の理由でレベルアップできない -->
			{#if data.levelUpStatus.is_locked || !data.levelUpStatus.can_attempt}
				<p class="error">{data.levelUpStatus.message || '現在レベルアップはできません。'}</p>
				{#if data.levelUpStatus.is_locked}
					<p class="cooldown-timer"> <span class="timer">{countdownDisplay}</span></p>
				{/if}

			<!-- Case 2: レベルアップ可能 -->
			{:else}
				{#if browser}
					<form
						id="level-up-form"
						method="POST"
						use:enhance={async (data) => {
							isLoading = true;
							clientMessage = '';
							console.log('[DIAG] use:enhance triggered. Form submission starting.');

							// フォーム送信が開始される直前に、非同期でフィンガープリントデータを取得します。
							console.log('[DIAG] Getting fingerprint data...');
							const fpData = await getRawFingerprintData();
							console.log('[DIAG] Fingerprint data obtained:', !!fpData);

							// 取得したデータをFormDataに追加します。
							data.formData.set('fingerprint_data', JSON.stringify(fpData));

							// フォーム送信完了後に実行されるapply関数を返します。
							return async ({ update }) => {
								console.log('[DIAG] Form submission complete. Resetting Turnstile and invalidating data.');
								if (widgetId && window.turnstile) {
									window.turnstile.reset(widgetId);
									turnstileToken = '';
								}
								await invalidateAll();
								await update();
								isLoading = false;
							};
						}}
					>
						<input type="hidden" name="turnstile_token" bind:value={turnstileToken} />
						<div bind:this={turnstileContainer} />
						<button
							type="submit"
							disabled={isLoading || !turnstileToken}
							class="level-up-button"
						>
							{isLoading ? '検証中...' : 'レベルを上げる'}
						</button>
					</form>
				{:else}
					<p>検証機能の準備をしています...</p>
				{/if}
			{/if}
		{:else}
			<!-- サーバーからデータをロード中の表示 -->
			<p>レベルアップの状態を確認しています...</p>
		{/if}
	</div>

	<hr />

	<RegenerateToken />

	<hr />

	<div class="mypage-menu">
		<h2>メニュー</h2>
		<ul class="menu-list">
			<li><a href="/my/bans" class="menu-link">BAN一覧</a></li>
			<li><a href="/blog/kiji" class="menu-link">専用ブラウザについて</a></li>
			<li><a href="/terms" class="menu-link">利用規約及びプライバシーポリシー</a></li>
		</ul>
	</div>
{:else}
	<p>情報を表示するには<a href="/auth/register">認証</a>してください。</p>
{/if}

<style>
	.user-status {
		margin-bottom: 2rem;
		padding: 1rem;
		background-color: #f0f8ff;
		border: 1px solid #cce5ff;
		border-radius: 4px;
		color: #004085;
	}

	.level-up-section {
		margin-top: 2rem;
		padding: 1.5rem;
		border: 1px solid #ccc;
		border-radius: 8px;
		max-width: 500px;
	}
	.level-up-button {
		margin-top: 1rem;
		padding: 0.6rem 1.2rem;
		font-size: 1rem;
	}
	.success {
		color: #155724;
		background-color: #d4edda;
		border-color: #c3e6cb;
		padding: 0.75rem;
		border-radius: 4px;
		margin-top: 1rem;
	}
	.error {
		color: #721c24;
		background-color: #f8d7da;
		border-color: #f5c6cb;
		padding: 0.75rem;
		border-radius: 4px;
		margin-top: 1rem;
	}
	.cooldown-timer {
		text-align: center;
	}
	.cooldown-timer .timer {
		font-size: 2rem;
		font-weight: bold;
		font-family: monospace;
		color: #c0392b;
		margin-top: 0.5rem;
		background-color: #fdf2f2;
		padding: 0.5rem;
		border-radius: 4px;
	}
	hr {
		border: none;
		border-top: 1px solid #eee;
		margin: 2rem 0;
	}

	.mypage-menu {
		margin-top: 2rem;
		max-width: 500px;
	}

	.mypage-menu h2 {
		font-size: 1.5rem;
		border-bottom: 2px solid #eee;
		padding-bottom: 0.5rem;
		margin-bottom: 1rem;
	}

	.menu-list {
		list-style: none;
		padding: 0;
		margin: 0;
	}
	.menu-list li + li {
		margin-top: 0.5rem;
	}

	.menu-link {
		display: block;
		padding: 0.75rem 1rem;
		border: 1px solid #ddd;
		border-radius: 4px;
		text-decoration: none;
		color: #007bff;
		font-weight: 500;
		transition: background-color 0.2s;
	}
	.menu-link:hover {
		background-color: #f8f9fa;
	}
</style>