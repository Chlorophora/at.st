import type { Post, Comment } from '../app';

/**
 * 投稿やコメントのレベル情報から、専ブラやdatで表示するための文字列を生成します。
 * 例: `Lv:12` や `Lv:12→13`
 * @param entity Post または Comment オブジェクト
 * @returns 表示用のレベル文字列。表示すべきでない場合は空文字列を返す。
 */
export function getLevelDisplayString(entity: Post | Comment | null | undefined): string {
	// 作成時レベルがnull（非表示対象）の場合は何も表示しない
	if (entity?.level_at_creation == null) {
		return '';
	}

	const baseDisplay = `Lv:${entity.level_at_creation}`;

	// 現在レベルが表示可能で、かつ作成時と異なる場合
	if (entity.level != null && entity.level !== entity.level_at_creation) {
		return `${baseDisplay}→${entity.level}`;
	}

	// 現在レベルが閾値によって隠されている場合
	if (entity.is_current_level_hidden) {
		return `${baseDisplay}→?`;
	}

	// それ以外（作成時と現在レベルが同じ）の場合
	return `${baseDisplay}`;
}
