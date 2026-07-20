// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions } from "vscode-languageclient/node";

let client: LanguageClient | undefined;

/** Activates the Vell VS Code extension. */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const serverOptions: ServerOptions = { command: "vell-lsp", args: [] };
  const clientOptions: LanguageClientOptions = { documentSelector: [{ scheme: "file", language: "vell" }] };
  client = new LanguageClient("vell", "Vell Language Server", serverOptions, clientOptions);
  await client.start();
  context.subscriptions.push(vscode.commands.registerCommand("vell.formatDocument", () => vscode.commands.executeCommand("editor.action.formatDocument")));
  context.subscriptions.push(vscode.commands.registerCommand("vell.exportHtml", async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;
    const doc = await vscode.workspace.openTextDocument({ content: editor.document.getText(), language: "html" });
    await vscode.window.showTextDocument(doc);
  }));
}

/** Deactivates the extension and stops the language client. */
export function deactivate(): Promise<void> | undefined { return client?.stop(); }
