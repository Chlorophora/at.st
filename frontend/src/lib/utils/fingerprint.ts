import FingerprintJS, {
	type Agent,
	type Component,
	type GetResult
} from '@fingerprintjs/fingerprintjs';
import { browser } from '$app/environment';

// FingerprintJSのインスタンスをキャッシュするための変数
// Agent型を明示的に指定することで、fpインスタンスの型安全性を確保します。
let fpPromise: Promise<Agent> | null = null;

// 偽装されにくく、ハードウェア特性(GPU, サウンドカード等)を強く反映するコンポーネントに限定
const STABLE_COMPONENTS = [
	'webgl',
	'canvas',
	'audio'
	// --- 以下のコンポーネントは安定性や一意性の観点から除外 ---
	// timezone: ユーザーが移動すると変わるため不安定
	// plugins, fonts, userAgent, platform: ブラウザの更新で変化しやすいため不安定
	// touchSupport: デバイスのクラスを特定するが、個々のデバイスの識別には寄与しにくい
];

/**
 * 文字列をSHA-256でハッシュ化し、16進数文字列として返すヘルパー関数
 * @param message ハッシュ化する文字列
 */
async function sha256(message: string): Promise<string> {
	// 文字列をエンコード
	const msgBuffer = new TextEncoder().encode(message);
	// ハッシュを計算
	const hashBuffer = await crypto.subtle.digest('SHA-256', msgBuffer);
	// バッファを16進数文字列に変換
	const hashArray = Array.from(new Uint8Array(hashBuffer));
	const hashHex = hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
	return hashHex;
}

/**
 * ユーザーのブラウザフィンガープリントを非同期で取得します。
 * 偽装されにくいコンポーネントのみを組み合わせて、より堅牢なIDを生成します。
 */
export async function getVisitorId(): Promise<string> {
	// サーバーサイドで実行された場合は、エラーを発生させずに空のIDを返す
	if (!browser) {
		console.warn('getVisitorId was called on the server. Returning empty string.');
		return '';
	}

	if (!fpPromise) { // 初回呼び出し時にクライアントでのみロード
		fpPromise = FingerprintJS.load();
	}
	const fp: Agent = await fpPromise;
	const result: GetResult = await fp.get({ extendedResult: true });

	// 安定したコンポーネントの値のみを抽出・結合
	const stableValues = STABLE_COMPONENTS.map(key => {
		const component = result.components[key] as Component<any>;
		if (component && component.value) {
			// 複雑なオブジェクトはJSON文字列化して一貫性を保つ
			return typeof component.value === 'object' ? JSON.stringify(component.value) : String(component.value);
		}
		return '';
	}).join(';');

	// 結合した値からハッシュを生成
	const visitorId = await sha256(stableValues);
	return visitorId;
}

/**
 * ユーザーのブラウザフィンガープリントの生データを非同期で取得します。
 * バックエンドでの詳細な検証（コンポーネントごとのハッシュ化など）に使用するためのものです。
 * @returns {Promise<GetResult>} FingerprintJSの`get`メソッドが返す、型付けされた結果オブジェクト。
 */
export async function getRawFingerprintData(): Promise<GetResult> {
	// サーバーサイドで実行された場合は、エラーを発生させずに空のコンポーネントを持つ結果を返す
	if (!browser) {
		console.warn('getRawFingerprintData was called on the server. Returning empty result.');
		return { visitorId: '', components: {}, confidence: { score: 0 } };
	}

	if (!fpPromise) { // 初回呼び出し時にクライアントでのみロード
		fpPromise = FingerprintJS.load();
	}
	const fp: Agent = await fpPromise;
	const result: GetResult = await fp.get({ extendedResult: true });
	return result;
}