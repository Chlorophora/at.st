import { writable, type Writable } from 'svelte/store';

// ユーザー情報の型を定義します
// app.d.ts の User 型と一致させ、一貫性を保ちます。
export interface User {
	user_id: number;
	email: string;
	role: 'user' | 'moderator' | 'admin';
	level: number;
}

// ユーザー情報を保持するストアを作成します (ログインしていない場合は null)
export const user: Writable<User | null> = writable(null);

// 複数回APIを呼ばないようにするためのフラグ
let isFetching = false;

// APIからユーザー情報を取得する関数
export async function fetchUser() {
	if (isFetching) return;
	
	isFetching = true;
	try {
		// hooks.server.ts のプロキシを経由させるため、相対パスを使用します。
		// これにより、CORSエラーを回避し、認証情報も正しく引き継がれます。
		const response = await fetch('/api/auth/me');

		if (response.ok) {
			const userData: User = await response.json();
			user.set(userData);
		} else {
			// 認証されていない場合などは null を設定
			user.set(null);
		}
	} catch (error) {
		console.error('Failed to fetch user:', error);
		user.set(null);
	} finally {
		isFetching = false;
	}
}