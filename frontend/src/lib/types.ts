// From backend/src/models.rs Board struct
export interface Board {
	id: number;
	name: string;
	description: string;
	default_name: string;
	created_at: string; // ISO 8601 date string
	updated_at: string;
	deleted_at: string | null;
	created_by: number | null;
	last_activity_at: string;
	archived_at: string | null;
}

// From backend/src/models.rs PostResponse struct
export interface Post {
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
	level: number | null;
}

// From backend/src/models.rs CommentResponse struct
export interface Comment {
	id: number;
	body: string;
	post_id: number;
	user_id: number | null;
	author_name: string | null;
	created_at: string;
	updated_at: string;
	display_user_id: string | null;
	permanent_user_hash: string | null;
	permanent_ip_hash: string | null;
	permanent_device_hash: string | null;
	level_at_creation: number | null;
	level: number | null;
}

// From backend/src/models.rs BanDetails struct
export interface BanDetails {
	id: number;
	ban_type: 'User' | 'Ip' | 'Device';
	hash_value: string;
	board_id: number | null;
	board_name: string | null;
	reason: string | null;
	created_by: number | null;
	created_by_email: string | null;
	created_at: string;
	expires_at: string | null;
	source_post_id: number | null;
	source_comment_id: number | null;
	source_email: string | null;
	source_ip_address: string | null;
	source_device_info: string | null;
}

// From backend/src/lib.rs get_identity_details
export interface IdentityDetails {
	email: string;
	ip_address: string;
	device_info: string;
	permanent_user_hash: string | null;
	permanent_ip_hash: string | null;
	permanent_device_hash: string | null;
}
