const vscode = require("vscode");
const path = require("path");
const fs = require("fs");
const os = require("os");
const { execFile } = require("child_process");
const { promisify } = require("util");
const { LanguageClient } = require("vscode-languageclient/node");

const execFileAsync = promisify(execFile);
const LANGUAGE_ID = "velox";
let client;
let starting = false;

const output = vscode.window.createOutputChannel("Velox Language Server");

function findProjectRoot(start) {
    let current = start;
    while (true) {
        if (fs.existsSync(path.join(current, "Cargo.toml"))) {
            return current;
        }
        const parent = path.dirname(current);
        if (parent === current) return null;
        current = parent;
    }
}

function getProjectRoot() {
    const folders = vscode.workspace.workspaceFolders || [];
    const active = vscode.window.activeTextEditor;
    const activeFolder = active
        ? vscode.workspace.getWorkspaceFolder(active.document.uri)
        : null;

    const candidates = [
        ...(activeFolder ? [activeFolder] : []),
        ...folders.filter(f => !activeFolder || f.uri.toString() !== activeFolder.uri.toString())
    ];

    for (const folder of candidates) {
        const found = findProjectRoot(folder.uri.fsPath);
        if (found) return found;
    }
    return null;
}

async function buildServer(projectRoot) {
    const cargoCandidates = [
        process.env.CARGO,
        path.join(os.homedir(), ".cargo", "bin", "cargo"),
        "cargo"
    ].filter(Boolean);

    let lastError;

    for (const cargo of [...new Set(cargoCandidates)]) {
        try {
            output.appendLine(`Running: ${cargo} build --release --bin velox-lsp`);
            const result = await execFileAsync(
                cargo,
                ["build", "--release", "--bin", "velox-lsp"],
                { cwd: projectRoot, maxBuffer: 10 * 1024 * 1024 }
            );
            if (result.stdout) output.appendLine(result.stdout);
            if (result.stderr) output.appendLine(result.stderr);
            return;
        } catch (error) {
            lastError = error;
            if (error.code !== "ENOENT") {
                if (error.stdout) output.appendLine(error.stdout);
                if (error.stderr) output.appendLine(error.stderr);
                throw error;
            }
        }
    }

    throw lastError || new Error("Could not find Cargo");
}

async function activate(context) {
    if (client || starting) return;
    starting = true;

    try {
        const projectRoot = getProjectRoot();
        if (!projectRoot) {
            throw new Error("Open a Velox project folder containing Cargo.toml.");
        }

        const executable = process.platform === "win32"
            ? "velox-lsp.exe"
            : "velox-lsp";

        const serverPath = path.join(
            projectRoot, "target", "release", executable
        );

        if (!fs.existsSync(serverPath)) {
            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: "Building Velox Language Server",
                cancellable: false
            }, () => buildServer(projectRoot));
        }

        if (!fs.existsSync(serverPath)) {
            throw new Error(`Language server binary not found: ${serverPath}`);
        }

        const serverOptions = {
            command: serverPath,
            args: [],
            options: { cwd: projectRoot }
        };

        const clientOptions = {
            documentSelector: [
                { scheme: "file", language: LANGUAGE_ID }
            ],
            outputChannel: output,
            traceOutputChannel: output
        };

        client = new LanguageClient(
            "veloxLanguageServer",
            "Velox Language Server",
            serverOptions,
            clientOptions
        );

        context.subscriptions.push(client);
        await client.start();

        output.appendLine("Velox Language Server started.");
    } catch (error) {
        output.appendLine(String(error));
        output.show(true);
        vscode.window.showErrorMessage(
            `Velox Language Server: ${error.message || error}`
        );
    } finally {
        starting = false;
    }
}

function deactivate() {
    if (!client) return undefined;
    const stopping = client.stop();
    client = undefined;
    return stopping;
}

module.exports = { activate, deactivate };
