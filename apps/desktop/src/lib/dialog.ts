import { ask } from "@tauri-apps/plugin-dialog";

/**
 * 跨平台确认对话框。
 *
 * 不用 window.confirm：macOS WKWebView 未实现该 API（调用返回 undefined、视为取消），
 * 会导致删除分类、重新编译等操作静默失效。plugin-dialog 的 ask() 走原生对话框。
 */
export async function confirmAction(message: string): Promise<boolean> {
    return ask(message, { title: "确认操作", kind: "warning" });
}
