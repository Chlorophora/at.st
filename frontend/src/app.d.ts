// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	// プロジェクト全体で利用する型をここに定義します

	// ユーザー情報の型
	type CreatorInfo = {
		display_user_id: string;
		level: number;
		level_at_creation: number;
	};

	type User = {
		user_id: number; // バックエンドの i32 (数値) と型を合わせます
		email: string;
		role: 'admin' | 'user' | 'moderator'; // プロジェクトに存在するロールを記述
		level: number; // ユーザーのレベル
		is_rate_limit_exempt: boolean; // 管理者がレート制限を免除されているか
		banned_from_level_up: boolean; // レベルアップが禁止されているか
		last_linking_token_generated_at: string | null; // 専ブラ連携トークンの最終発行日時
	};

	// 板情報の型
	type Board = {
		id: number;
		name: string;
		description: string;
		default_name: string;
		created_by: number | null; // 板作成者のユーザーID (nullの場合もある)
		created_at: string; // 板の作成日時
		updated_at: string;
		deleted_at: string | null;
		last_activity_at: string;
		archived_at: string | null;
		max_posts: number; // 板のスレッド数上限
		moderation_type: 'alpha' | 'beta';
		auto_archive_enabled: boolean;
		can_moderate?: boolean; // 管理者または板作成者かどうか
		proxycheck_json?: object | null;
	};

	// 投稿に埋め込まれる板情報のサマリー
	type BoardSummary = {
		id: number;
		name: string;
	};

	// 投稿情報の型 (バックエンドの PostResponse に対応)
	type Post = {
		id: number;
		title: string;
		body: string;
		author_name: string | null;
		created_at: string;
		updated_at: string;
		board_id: number | null;
		deleted_at: string | null;
		user_id: number | null;
		archived_at: string | null;
		last_activity_at: string;
		display_user_id: string | null;
		permanent_user_hash: string | null;
		permanent_ip_hash: string | null;
		permanent_device_hash: string | null;
		level_at_creation: number | null;
		level: number | null; // 投稿者の現在のレベル
		is_current_level_hidden?: boolean; // 現在のレベルが閾値によって隠されているか
		response_count?: number; // スレッド一覧でレス数を表示するために追加
		momentum?: number; // 勢い (レス/日)
		can_moderate?: boolean; // モデレート権限があるか
		proxycheck_json?: object | null;
	};

	// コメント情報の型 (バックエンドの CommentResponse に対応)
	type Comment = {
		id: number;
		post_id: number;
		body: string;
		author_name: string | null;
		created_at: string;
		updated_at: string;
		user_id: number | null;
		display_user_id: string | null;
		permanent_user_hash: string | null;
		permanent_ip_hash: string | null;
		permanent_device_hash: string | null;
		level_at_creation: number | null;
		level: number | null; // 投稿者の現在のレベル
		is_current_level_hidden?: boolean; // 現在のレベルが閾値によって隠されているか
		post_title?: string | null; // ID検索結果で利用
		response_number?: number; // レスナンバー
		can_moderate?: boolean; // モデレート権限があるか
		proxycheck_json?: object | null;
	};

	// 過去ログ検索結果の型 (バックエンドの ArchivedPostItem に対応)
	type ArchivedPost = {
		id: number;
		title: string;
		body: string;
		author_name: string | null;
		created_at: string;
		updated_at: string;
		board_id: number | null;
		deleted_at: string | null;
		archived_at: string | null;
		last_activity_at: string | null;
		total_responses: number;
		board_name: string | null;
	};

	// BAN情報の型 (バックエンドの BanResponse に対応)
	type Ban = {
		id: number;
		ban_type: 'User' | 'Ip' | 'Device';
		hash_value: string;
		reason: string | null;
		expires_at: string | null;
		created_at: string;
		created_by: number;
		board_id: number | null;
		post_id: number | null;
		scope: 'Global' | 'Board' | 'Thread';
		scope_display_name: string;
		source_post_id: number | null;
		source_comment_id: number | null;
		created_by_email?: string; // 管理者向け情報
		board_name?: string | null; // 管理者向け情報
		post_title?: string | null; // 管理者向け情報
		source_email?: string | null;
		source_ip_address?: string | null;
		source_device_info?: string | null;
		source_user_id?: number | null;
	};

	// 認証・レベルアップ履歴の型 (バックエンドの VerificationHistoryItem に対応)
	type VerificationHistoryItem = {
		id: number;
		attempt_type: 'registration' | 'level_up' | null;
		is_success: boolean;
		ip_address: string;
		created_at: string;
		rejection_reason: string | null;
		// JSON.parse() する前のValue型を想定
		fingerprint_json: object | null;
		proxycheck_json: object | null;
	};

	// 管理者向けに表示される個人情報の型
	type IdentityDetails = {
		email: string | null;
		ip_address: string | null;
		device_info: string | null;
		permanent_user_hash: string | null;
		permanent_ip_hash: string | null;
		permanent_device_hash: string | null;
	};

	// レベルアップステータスの型
	type LevelUpStatus = {
		can_attempt: boolean;
		is_locked: boolean;
		lock_expires_in_seconds: number | null;
		message: string;
	};
	
	// レート制限の監視対象 (バックエンドのenumに対応)
	type RateLimitTarget =
		| 'UserId'
		| 'IpAddress'
		| 'DeviceId'
		| 'UserAndIp'
		| 'UserAndDevice'
		| 'IpAndDevice'
		| 'All';

	// レート制限の対象アクション (バックエンドのenumに対応)
	type RateLimitActionType = 'CreateBoard' | 'CreatePost' | 'CreateComment';

	// レート制限ルールの型 (バックエンドのモデルに対応)
	type RateLimitRule = {
		id: number;
		name: string; // ルールを識別するための名前 (例: 「同一IPからの連続投稿」)
		target: RateLimitTarget;
		action_type: RateLimitActionType; // ルールがどの操作を対象とするか
		threshold: number; // A回
		time_frame_seconds: number; // X時間Y分Z秒 (秒に変換)
		lockout_seconds: number; // B時間C分D秒 (秒に変換)
		is_enabled: boolean;
		created_at: string;
		updated_at: string;
		created_by: number;
		created_by_email?: string; // 管理者向けに表示
	};

	// レート制限ロック情報の型
	type RateLimitLock = {
		target_key: string;
		expires_at: string;
		rule_id: number;
		rule_name: string | null; // ルールが削除されている場合はnull
	};

	// ユーザー向けID検索機能のレスポンス型
	type HistoryItem =
		| {
				type: 'Post';
				data: Post;
		  }
		| {
				type: 'Comment';
				data: Comment;
		  };

	type HistorySummary = {
		first_seen: string | null;
		last_seen: string | null;
		created_thread_count: number;
		comment_count: number;
		total_contribution_count: number;
		// [title, count]
		created_threads: [string, number][];
		// [title, count]
		commented_in_threads: [string, number][];
	};

	type HistoryResponse = {
		summary: HistorySummary;
		items: HistoryItem[];
	};


	namespace App {
		// interface Error {}
		interface Locals {
			cookiesToSet?: string[]; // フックでSet-Cookieヘッダーを設定するために使用
			sjisBodyBuffer?: ArrayBuffer; // 専ブラからのShift_JISリクエストボディを格納
		}
		interface PageData {
			user?: User | null; // すべてのページの`data`プロパティでuserが利用可能になる
			boards?: Board[]; // 板一覧ページで利用
			board?: Board; // 板詳細ページで利用
			post?: Post; // スレッド詳細ページで利用
			posts?: Post[]; // any[] から具体的な Post[] 型に変更
			comments?: Comment[]; // スレッド詳細ページで利用 (この行を追加または確認)
			email?: string; // /auth/verify-otp ページで email を受け取るために追加
			bans?: Ban[]; // BAN一覧ページで利用
			creatorInfo?: CreatorInfo | null; // 板詳細ページで管理者向けに利用
			levelUpStatus?: LevelUpStatus; // マイページで利用
			rateLimitRules?: RateLimitRule[]; // レート制限管理ページで利用
			rateLimitLocks?: RateLimitLock[]; // レート制限管理ページで利用
			paginatedBoards?: { items: Board[]; total_count: number };
			historyResponse?: HistoryResponse; // ID検索ページで利用

			// Admin settings
			settings?: { [key: string]: any };
			// /archive ページ用のデータ
			archivedPosts?: ArchivedPost[];
			totalCount?: number;
			currentPage?: number;
			limit?: number;
			searchParams?: Record<string, string | boolean>;
			paginatedResponse?: PaginatedResponse<any>; // 汎用的なページネーションレスポンス
		}
		// interface PageState {}
		// interface Platform {}
	}
}

export {};
