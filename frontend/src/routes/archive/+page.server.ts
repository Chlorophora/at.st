import type { PageServerLoad } from './$types';
import { error as svelteKitError } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ url, fetch, parent }) => {
    // 親レイアウトの`load`が完了するのを待ち、ユーザーセッションの変更を検知できるようにする
    await parent();

    // 1. ページのURLからすべてのクエリパラメータを取得します。
    //    url.searchParams は `search_type=or` などを含んだURLSearchParamsオブジェクトです。
    const queryParams = new URLSearchParams(url.searchParams);

    // 2. ページネーション用のパラメータを設定または上書きします。
    const page = parseInt(queryParams.get('page') || '1');
    const limit = 50; // 1ページあたりの件数
    const offset = (page - 1) * limit;
    queryParams.set('limit', limit.toString());
    queryParams.set('offset', offset.toString());

    try {
        // 3. 組み立てたクエリパラメータを使ってバックエンドAPIにリクエストを送信します。
        //    これにより、`search_type` を含むすべてのフィルタ条件がバックエンドに渡されます。
        const response = await fetch(`/api/archive?${queryParams.toString()}`);
        if (!response.ok) {
            // APIからの応答がエラーだった場合、SvelteKitのエラーとしてスローします
            throw svelteKitError(response.status, `Failed to fetch archived posts: ${response.statusText}`);
        }
        const data: { items: ArchivedPost[]; total_count: number } = await response.json();

        // 4. フロントエンドコンポーネントに渡すデータを準備します。
        //    URLのパラメータをオブジェクトに変換して、Svelteコンポーネントで使いやすくします。
        const searchParamsForClient: Record<string, string | boolean> = {};
		// まずは全てのパラメータを文字列としてコピー
		for (const [key, value] of url.searchParams.entries()) {
			searchParamsForClient[key] = value;
		}

		// 次に、booleanとして扱いたいパラメータを正しい型で上書き/設定する
		// 'include_author_names' は 'true' の文字列の場合のみ true, それ以外は false
		searchParamsForClient['include_author_names'] = url.searchParams.get('include_author_names') === 'true';

		// 'include_active_threads' は 'false' の文字列の場合のみ false, それ以外 (未指定含む) は true
		searchParamsForClient['include_active_threads'] = url.searchParams.get('include_active_threads') !== 'false';

		// 'show_deleted' は 'true' の文字列の場合のみ true, それ以外は false
		searchParamsForClient['show_deleted'] = url.searchParams.get('show_deleted') === 'true';

        return {
            archivedPosts: data.items,
            totalCount: data.total_count,
            currentPage: page,
            limit,
            // すべての検索パラメータをコンポーネントに渡し、UIの状態を正しく復元できるようにします。
            searchParams: searchParamsForClient
        };
    } catch (error) {
        console.error('Failed to load archived posts:', error);
        // 既にSvelteKitのエラーオブジェクトならそのままスローし、そうでなければ500エラーを生成します
		if (error && typeof error === 'object' && 'status' in error) {
			throw error;
		}
        throw svelteKitError(500, '過去ログデータの読み込み中に予期せぬエラーが発生しました。');
    }
};