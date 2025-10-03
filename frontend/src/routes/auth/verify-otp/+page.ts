import type { PageLoad } from './$types';

export const load: PageLoad = ({ url }) => {
	const email = url.searchParams.get('email');
	if (!email) {
		// URLにemailパラメータがない場合はエラーを返すか、登録ページにリダイレクトさせます。
		return {
			status: 400,
			error: new Error('Email parameter is missing')
		};
	}
	return { email };
};