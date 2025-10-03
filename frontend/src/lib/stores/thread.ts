import { writable } from 'svelte/store';

// number | null 型のストアを作成
// number: レス数, null: レス数が未設定（スレッドページ以外）
export const currentThreadResponseCount = writable<number | null>(null);
