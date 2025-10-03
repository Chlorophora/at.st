import { handler } from './build/handler.js';
import http from 'http';

const port = process.env.PORT || 3000;

http
	.createServer(async (req, res) => {
		// すべてのリクエストをSvelteKitのハンドラに直接渡します。
		return handler(req, res);
	})
	.listen(port, () => {
		console.log(`> Node.js server listening on port ${port}`);
	});