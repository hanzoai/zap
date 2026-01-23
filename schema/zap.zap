@0xb5e7e8d3c9f2a1b4;

# ZAP: Zero-copy Agent Protocol
# Cap'n Proto RPC Edition - The "MCP Killer"
#
# Version: 0.2.1
# Profile: zap-capnp-1
# Spec: /docs/ZAP-WHITEPAPER.md
#
# "One ZAP endpoint to rule all MCP servers."
#
# ZAP Gateway auto-wraps any number of MCP servers into one unified,
# consensus-aware, zero-copy tool mesh. Agents speak ZAP once;
# everything else is implementation detail.
#
# Features:
# - Drop-in Claude Code replacement (all canonical tools)
# - REPL/Notebook/Browser interactive environments
# - Unified Catalog across all MCP servers
# - Lux metastable consensus for stable discovery
# - Zero-copy performance (wire format = memory format)

# ============================================================================
# ZAP Profile Annotations
# ============================================================================

annotation namespace @0xa1b2c3d4e5f6a7b8 (file) :Text;
annotation version @0xa1b2c3d4e5f6a7b9 (file) :Text;
annotation protocol @0xa1b2c3d4e5f6a7ba (file) :Text;
annotation profile @0xa1b2c3d4e5f6a7bb (file) :Text;

annotation effect @0xa1b2c3d4e5f6a7bc (method) :Effect;
annotation idempotent @0xa1b2c3d4e5f6a7bd (method) :Bool;
annotation replayable @0xa1b2c3d4e5f6a7be (method) :Bool;
annotation scope @0xa1b2c3d4e5f6a7bf (method) :Scope;
annotation witness @0xa1b2c3d4e5f6a7c0 (method) :WitnessLevel;
annotation costModel @0xa1b2c3d4e5f6a7c1 (method) :CostModel;

$namespace("ai.hanzo.zap");
$version("0.2.1");
$protocol("capnp-rpc");
$profile("zap-capnp-1");

# ============================================================================
# Core Enums
# ============================================================================

enum Effect {
    pure @0;              # No side effects, deterministic, cacheable
    deterministic @1;     # Side effects, but reproducible given same input
    nondeterministic @2;  # May vary between calls (network, time, random)
}

enum Scope {
    span @0;       # Single operation
    file @1;       # File-level
    repo @2;       # Repository-level
    workspace @3;  # Workspace-level
    node @4;       # Node-level
    chain @5;      # Blockchain/consensus chain
    global @6;     # Global scope
}

enum WitnessLevel {
    none @0;       # No witness logging
    minimal @1;    # Basic operation logging
    full @2;       # Full witness with inputs/outputs
}

enum CostModel {
    free @0;       # No cost
    metered @1;    # Metered usage
    gas @2;        # Gas-based (blockchain)
    quota @3;      # Quota-based
}

# ============================================================================
# Common Types
# ============================================================================

struct Blob {
    mimeType @0 :Text;
    bytes @1 :Data;
}

struct Meta {
    timestamp @0 :UInt64;
    traceId @1 :Text;
    spanId @2 :Text;
}

struct Annotations {
    audience @0 :List(Text);
    priority @1 :Float64;
    tags @2 :List(Text);
}

struct ContentBlock {
    union {
        text @0 :Text;
        image @1 :Blob;
        audio @2 :Blob;
        video @3 :Blob;
        resourceLink @4 :ResourceLink;
        embedded @5 :ResourceContent;
    }
    annotations @6 :Annotations;
    meta @7 :Meta;
}

struct ResourceLink {
    uri @0 :Text;
    title @1 :Text;
}

struct Cursor {
    token @0 :Data;
}

# ============================================================================
# Streaming Primitives
# ============================================================================

interface ByteStream {
    readNext @0 (maxBytes :UInt64) -> (data :Data, done :Bool);
    cancel @1 () -> ();
}

interface EventStream {
    next @0 () -> (event :Event, done :Bool);
    cancel @1 () -> ();
}

struct Event {
    type @0 :Text;
    data @1 :Data;
    timestamp @2 :UInt64;
}

# ============================================================================
# Progress & Tasks
# ============================================================================

struct Progress {
    done @0 :UInt64;
    total @1 :UInt64;
    message @2 :Text;
}

interface ProgressSink {
    report @0 (progress :Progress) -> ();
}

enum TaskState {
    pending @0;
    running @1;
    completed @2;
    failed @3;
    cancelled @4;
}

struct TaskStatus {
    state @0 :TaskState;
    progress @1 :Progress;
    startedAt @2 :UInt64;
    updatedAt @3 :UInt64;
}

interface Task {
    status @0 () -> (status :TaskStatus);
    result @1 () -> (data :AnyPointer);
    cancel @2 () -> ();
    output @3 () -> (stream :EventStream);
}

# ============================================================================
# Bootstrap & Negotiation
# ============================================================================

struct Implementation {
    name @0 :Text;
    version @1 :Text;
}

struct ClientCaps {
    roots @0 :Bool;
    sampling @1 :Bool;
    elicitation @2 :Bool;
    experimental @3 :List(Text);
}

struct EndpointCaps {
    tools @0 :Bool;
    resources @1 :Bool;
    prompts @2 :Bool;
    tasks @3 :Bool;
    logging @4 :Bool;
    repl @5 :Bool;
    notebook @6 :Bool;
    browser @7 :Bool;
    catalog @8 :Bool;
    coordination @9 :Bool;
    experimental @10 :List(Text);
}

struct Hello {
    protocolVersion @0 :Text;
    clientInfo @1 :Implementation;
    capabilities @2 :ClientCaps;
    schemaHash @3 :Data;
}

struct Welcome {
    protocolVersion @0 :Text;
    endpointInfo @1 :Implementation;
    capabilities @2 :EndpointCaps;
    instructions @3 :Text;
    schemaHash @4 :Data;
}

# ============================================================================
# Call Context (shared by all tool calls)
# ============================================================================

struct CallContext {
    traceId @0 :Text;
    spanId @1 :Text;
    timeout @2 :UInt64;
    progress @3 :ProgressSink;
    determinism @4 :DeterminismContext;
}

struct DeterminismContext {
    timestamp @0 :UInt64;
    randomSeed @1 :Data;
    chainHeight @2 :UInt64;
}

# ============================================================================
# Error Types
# ============================================================================

struct ZapError {
    code @0 :ErrorCode;
    message @1 :Text;
    details @2 :Data;
}

enum ErrorCode {
    unknownAction @0;
    invalidParams @1;
    notFound @2;
    conflict @3;
    permissionDenied @4;
    timeout @5;
    internalError @6;
    rateLimited @7;
    notConnected @8;
    protocolError @9;
}

# ============================================================================
# CANONICAL TOOL: fs (Filesystem)
# Drop-in for: Read, Write, Edit, Glob
# ============================================================================

struct FsReadRequest {
    path @0 :Text;
    offset @1 :UInt64;
    limit @2 :UInt64;
}

struct FsReadResult {
    content @0 :Text;
    mimeType @1 :Text;
    size @2 :UInt64;
    lines @3 :UInt64;
}

struct FsWriteRequest {
    path @0 :Text;
    content @1 :Text;
    createDirs @2 :Bool;
}

struct FsEditRequest {
    path @0 :Text;
    oldText @1 :Text;
    newText @2 :Text;
    replaceAll @3 :Bool;
}

struct FsGlobRequest {
    pattern @0 :Text;
    path @1 :Text;
}

struct FsSearchRequest {
    pattern @0 :Text;
    path @1 :Text;
    glob @2 :Text;
    ignoreCase @3 :Bool;
    maxResults @4 :UInt32;
    contextLines @5 :UInt32;
}

struct FsSearchMatch {
    file @0 :Text;
    line @1 :UInt32;
    column @2 :UInt32;
    content @3 :Text;
    context @4 :Text;
}

struct FsTreeRequest {
    path @0 :Text;
    depth @1 :UInt32;
    showHidden @2 :Bool;
}

struct FsStatResult {
    path @0 :Text;
    size @1 :UInt64;
    isDir @2 :Bool;
    isFile @3 :Bool;
    modified @4 :UInt64;
    created @5 :UInt64;
    permissions @6 :Text;
}

interface Fs {
    read @0 (req :FsReadRequest, ctx :CallContext) -> (result :FsReadResult)
        $effect(deterministic)
        $idempotent(true);

    write @1 (req :FsWriteRequest, ctx :CallContext) -> (path :Text)
        $effect(nondeterministic);

    edit @2 (req :FsEditRequest, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    glob @3 (req :FsGlobRequest, ctx :CallContext) -> (paths :List(Text))
        $effect(deterministic)
        $idempotent(true);

    search @4 (req :FsSearchRequest, ctx :CallContext) -> (matches :List(FsSearchMatch))
        $effect(deterministic)
        $idempotent(true);

    tree @5 (req :FsTreeRequest, ctx :CallContext) -> (tree :Text)
        $effect(deterministic)
        $idempotent(true);

    stat @6 (path :Text, ctx :CallContext) -> (stat :FsStatResult)
        $effect(deterministic)
        $idempotent(true);

    mkdir @7 (path :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    remove @8 (path :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    copy @9 (src :Text, dst :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    move @10 (src :Text, dst :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);
}

# ============================================================================
# CANONICAL TOOL: proc (Process Execution)
# Drop-in for: Bash
# ============================================================================

struct ProcRunRequest {
    command @0 :Text;
    args @1 :List(Text);
    cwd @2 :Text;
    env @3 :List(EnvVar);
    timeout @4 :UInt64;
    stdin @5 :Text;
}

struct EnvVar {
    key @0 :Text;
    value @1 :Text;
}

struct ProcRunResult {
    exitCode @0 :Int32;
    stdout @1 :Text;
    stderr @2 :Text;
    durationMs @3 :UInt64;
}

interface Proc {
    run @0 (req :ProcRunRequest, ctx :CallContext) -> (result :ProcRunResult)
        $effect(nondeterministic);

    bg @1 (req :ProcRunRequest, ctx :CallContext) -> (task :Task)
        $effect(nondeterministic);

    signal @2 (pid :UInt32, signal :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    list @3 (ctx :CallContext) -> (processes :List(ProcessInfo))
        $effect(deterministic);
}

struct ProcessInfo {
    pid @0 :UInt32;
    name @1 :Text;
    status @2 :Text;
    cpu @3 :Float64;
    memory @4 :UInt64;
}

# ============================================================================
# CANONICAL TOOL: vcs (Version Control)
# Git operations
# ============================================================================

struct VcsStatusResult {
    branch @0 :Text;
    clean @1 :Bool;
    staged @2 :List(Text);
    modified @3 :List(Text);
    untracked @4 :List(Text);
    ahead @5 :UInt32;
    behind @6 :UInt32;
}

struct VcsDiffRequest {
    path @0 :Text;
    staged @1 :Bool;
    commit @2 :Text;
}

struct VcsCommitRequest {
    message @0 :Text;
    paths @1 :List(Text);
    amend @2 :Bool;
}

struct VcsLogEntry {
    hash @0 :Text;
    shortHash @1 :Text;
    author @2 :Text;
    date @3 :UInt64;
    message @4 :Text;
}

interface Vcs {
    status @0 (ctx :CallContext) -> (status :VcsStatusResult)
        $effect(deterministic)
        $idempotent(true);

    diff @1 (req :VcsDiffRequest, ctx :CallContext) -> (diff :Text)
        $effect(deterministic)
        $idempotent(true);

    commit @2 (req :VcsCommitRequest, ctx :CallContext) -> (hash :Text)
        $effect(nondeterministic);

    log @3 (count :UInt32, ctx :CallContext) -> (entries :List(VcsLogEntry))
        $effect(deterministic)
        $idempotent(true);

    branch @4 (name :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    checkout @5 (ref :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    stash @6 (ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    stashPop @7 (ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    add @8 (paths :List(Text), ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    reset @9 (paths :List(Text), ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);
}

# ============================================================================
# CANONICAL TOOL: code (Code Analysis)
# AST, symbols, transforms
# ============================================================================

struct CodeSymbol {
    name @0 :Text;
    kind @1 :SymbolKind;
    file @2 :Text;
    line @3 :UInt32;
    column @4 :UInt32;
    endLine @5 :UInt32;
    endColumn @6 :UInt32;
    signature @7 :Text;
}

enum SymbolKind {
    function @0;
    class @1;
    method @2;
    variable @3;
    constant @4;
    interface @5;
    type @6;
    enum @7;
    module @8;
    property @9;
}

struct CodeParseResult {
    language @0 :Text;
    ast @1 :Text;
    symbols @2 :List(CodeSymbol);
    errors @3 :List(ParseError);
}

struct ParseError {
    line @0 :UInt32;
    column @1 :UInt32;
    message @2 :Text;
}

interface Code {
    parse @0 (path :Text, ctx :CallContext) -> (result :CodeParseResult)
        $effect(pure)
        $idempotent(true);

    symbols @1 (path :Text, ctx :CallContext) -> (symbols :List(CodeSymbol))
        $effect(pure)
        $idempotent(true);

    references @2 (symbol :Text, path :Text, ctx :CallContext) -> (refs :List(CodeSymbol))
        $effect(deterministic)
        $idempotent(true);

    definition @3 (symbol :Text, path :Text, line :UInt32, ctx :CallContext) -> (def :CodeSymbol)
        $effect(deterministic)
        $idempotent(true);

    hover @4 (path :Text, line :UInt32, column :UInt32, ctx :CallContext) -> (info :Text)
        $effect(deterministic)
        $idempotent(true);

    completion @5 (path :Text, line :UInt32, column :UInt32, ctx :CallContext) -> (items :List(CompletionItem))
        $effect(deterministic);
}

struct CompletionItem {
    label @0 :Text;
    kind @1 :SymbolKind;
    detail @2 :Text;
    insertText @3 :Text;
}

# ============================================================================
# CANONICAL TOOL: net (Network)
# Drop-in for: WebFetch, WebSearch
# ============================================================================

struct NetFetchRequest {
    url @0 :Text;
    method @1 :Text;
    headers @2 :List(Header);
    body @3 :Data;
    timeout @4 :UInt64;
    followRedirects @5 :Bool;
}

struct Header {
    name @0 :Text;
    value @1 :Text;
}

struct NetFetchResult {
    status @0 :UInt16;
    headers @1 :List(Header);
    body @2 :Data;
    url @3 :Text;
}

struct NetSearchResult {
    title @0 :Text;
    url @1 :Text;
    snippet @2 :Text;
}

interface Net {
    fetch @0 (req :NetFetchRequest, ctx :CallContext) -> (result :NetFetchResult)
        $effect(nondeterministic);

    search @1 (query :Text, maxResults :UInt32, ctx :CallContext) -> (results :List(NetSearchResult))
        $effect(nondeterministic);

    download @2 (url :Text, path :Text, ctx :CallContext) -> (task :Task)
        $effect(nondeterministic);

    head @3 (url :Text, ctx :CallContext) -> (headers :List(Header), status :UInt16)
        $effect(nondeterministic);
}

# ============================================================================
# CANONICAL TOOL: repl (Interactive Sessions)
# Node.js, Python, Ruby, etc.
# ============================================================================

struct ReplSession {
    id @0 :Text;
    language @1 :Text;
    cwd @2 :Text;
    pid @3 :UInt32;
}

struct ReplEvalResult {
    output @0 :Text;
    error @1 :Text;
    result @2 :Text;
    displayData @3 :List(DisplayData);
}

struct DisplayData {
    mimeType @0 :Text;
    data @1 :Data;
}

interface Repl {
    # Start a new REPL session
    start @0 (language :Text, cwd :Text, ctx :CallContext) -> (session :ReplSession)
        $effect(nondeterministic);

    # Evaluate code in session
    eval @1 (sessionId :Text, code :Text, ctx :CallContext) -> (result :ReplEvalResult)
        $effect(nondeterministic);

    # Get session output stream
    output @2 (sessionId :Text) -> (stream :EventStream)
        $effect(nondeterministic);

    # Send input to session
    input @3 (sessionId :Text, text :Text, ctx :CallContext) -> ()
        $effect(nondeterministic);

    # List active sessions
    list @4 (ctx :CallContext) -> (sessions :List(ReplSession))
        $effect(deterministic);

    # Kill session
    kill @5 (sessionId :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Get session history
    history @6 (sessionId :Text, ctx :CallContext) -> (entries :List(Text))
        $effect(deterministic);

    # Interrupt running eval (Ctrl+C)
    interrupt @7 (sessionId :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);
}

# ============================================================================
# CANONICAL TOOL: notebook (Jupyter-style Notebooks)
# Any language via kernels
# ============================================================================

struct NotebookInfo {
    path @0 :Text;
    language @1 :Text;
    cellCount @2 :UInt32;
    kernelStatus @3 :KernelStatus;
}

enum KernelStatus {
    idle @0;
    busy @1;
    starting @2;
    dead @3;
}

struct NotebookCell {
    id @0 :Text;
    cellType @1 :CellType;
    source @2 :Text;
    outputs @3 :List(CellOutput);
    executionCount @4 :UInt32;
    metadata @5 :Data;
}

enum CellType {
    code @0;
    markdown @1;
    raw @2;
}

struct CellOutput {
    outputType @0 :OutputType;
    data @1 :List(DisplayData);
    text @2 :Text;
    ename @3 :Text;
    evalue @4 :Text;
    traceback @5 :List(Text);
}

enum OutputType {
    stream @0;
    displayData @1;
    executeResult @2;
    error @3;
}

interface Notebook {
    # Open/create notebook
    open @0 (path :Text, ctx :CallContext) -> (info :NotebookInfo)
        $effect(nondeterministic);

    # Read notebook
    read @1 (path :Text, ctx :CallContext) -> (cells :List(NotebookCell))
        $effect(deterministic);

    # Execute cell
    executeCell @2 (path :Text, cellId :Text, ctx :CallContext) -> (output :CellOutput)
        $effect(nondeterministic);

    # Execute all cells
    executeAll @3 (path :Text, ctx :CallContext) -> (task :Task)
        $effect(nondeterministic);

    # Edit cell
    editCell @4 (path :Text, cellId :Text, source :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Insert cell
    insertCell @5 (path :Text, afterCellId :Text, cellType :CellType, source :Text, ctx :CallContext) -> (cellId :Text)
        $effect(nondeterministic);

    # Delete cell
    deleteCell @6 (path :Text, cellId :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Restart kernel
    restartKernel @7 (path :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Interrupt kernel
    interruptKernel @8 (path :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Save notebook
    save @9 (path :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # List available kernels
    kernels @10 (ctx :CallContext) -> (kernels :List(KernelSpec))
        $effect(deterministic);
}

struct KernelSpec {
    name @0 :Text;
    displayName @1 :Text;
    language @2 :Text;
}

# ============================================================================
# CANONICAL TOOL: browser (Browser Console/DevTools)
# Chrome DevTools Protocol compatible
# ============================================================================

struct BrowserPage {
    id @0 :Text;
    url @1 :Text;
    title @2 :Text;
}

struct ConsoleMessage {
    level @0 :ConsoleLevel;
    text @1 :Text;
    url @2 :Text;
    line @3 :UInt32;
    column @4 :UInt32;
    timestamp @5 :UInt64;
}

enum ConsoleLevel {
    log @0;
    info @1;
    warn @2;
    error @3;
    debug @4;
}

struct EvalResult {
    value @0 :Text;
    type @1 :Text;
    className @2 :Text;
    description @3 :Text;
    exception @4 :Text;
}

struct DomNode {
    nodeId @0 :UInt32;
    nodeType @1 :UInt32;
    nodeName @2 :Text;
    localName @3 :Text;
    nodeValue @4 :Text;
    attributes @5 :List(Text);
    childCount @6 :UInt32;
}

interface Browser {
    # Connect to browser
    connect @0 (endpoint :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # List pages/tabs
    pages @1 (ctx :CallContext) -> (pages :List(BrowserPage))
        $effect(deterministic);

    # Navigate to URL
    navigate @2 (pageId :Text, url :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Evaluate JavaScript in page
    eval @3 (pageId :Text, expression :Text, ctx :CallContext) -> (result :EvalResult)
        $effect(nondeterministic);

    # Get console messages
    console @4 (pageId :Text) -> (stream :EventStream)
        $effect(nondeterministic);

    # Take screenshot
    screenshot @5 (pageId :Text, format :Text, ctx :CallContext) -> (data :Data)
        $effect(nondeterministic);

    # Get page HTML
    html @6 (pageId :Text, ctx :CallContext) -> (html :Text)
        $effect(deterministic);

    # Query DOM
    querySelector @7 (pageId :Text, selector :Text, ctx :CallContext) -> (node :DomNode)
        $effect(deterministic);

    querySelectorAll @8 (pageId :Text, selector :Text, ctx :CallContext) -> (nodes :List(DomNode))
        $effect(deterministic);

    # Click element
    click @9 (pageId :Text, selector :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Type text
    type @10 (pageId :Text, selector :Text, text :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Wait for selector
    waitFor @11 (pageId :Text, selector :Text, timeout :UInt64, ctx :CallContext) -> (found :Bool)
        $effect(nondeterministic);
}

# ============================================================================
# CANONICAL TOOL: agent (Sub-agent Spawning)
# Drop-in for: Task tool
# ============================================================================

struct AgentConfig {
    name @0 :Text;
    model @1 :Text;
    systemPrompt @2 :Text;
    tools @3 :List(Text);
    maxTurns @4 :UInt32;
    timeout @5 :UInt64;
}

struct AgentResult {
    output @0 :Text;
    toolCalls @1 :UInt32;
    turns @2 :UInt32;
    success @3 :Bool;
}

interface Agent {
    # Spawn a sub-agent
    spawn @0 (prompt :Text, config :AgentConfig, ctx :CallContext) -> (task :Task)
        $effect(nondeterministic);

    # List running agents
    list @1 (ctx :CallContext) -> (agents :List(AgentInfo))
        $effect(deterministic);

    # Send message to agent
    message @2 (agentId :Text, message :Text, ctx :CallContext) -> (response :Text)
        $effect(nondeterministic);

    # Kill agent
    kill @3 (agentId :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);
}

struct AgentInfo {
    id @0 :Text;
    name @1 :Text;
    status @2 :TaskState;
    turns @3 :UInt32;
    startedAt @4 :UInt64;
}

# ============================================================================
# CANONICAL TOOL: user (User Interaction)
# Drop-in for: AskUserQuestion, TodoWrite
# ============================================================================

struct Question {
    question @0 :Text;
    header @1 :Text;
    options @2 :List(QuestionOption);
    multiSelect @3 :Bool;
}

struct QuestionOption {
    label @0 :Text;
    description @1 :Text;
}

struct Todo {
    content @0 :Text;
    status @1 :TodoStatus;
    activeForm @2 :Text;
}

enum TodoStatus {
    pending @0;
    inProgress @1;
    completed @2;
}

interface User {
    # Ask user a question
    ask @0 (questions :List(Question), ctx :CallContext) -> (answers :List(Text))
        $effect(nondeterministic);

    # Update todo list
    todos @1 (todos :List(Todo), ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Notify user
    notify @2 (title :Text, message :Text, ctx :CallContext) -> ()
        $effect(nondeterministic);

    # Request confirmation
    confirm @3 (message :Text, ctx :CallContext) -> (confirmed :Bool)
        $effect(nondeterministic);
}

# ============================================================================
# CATALOG: Unified Tool Discovery
# MCP aggregation layer
# ============================================================================

struct ToolId {
    namespace @0 :Text;   # e.g. "native", "mcp.github", "mcp.stripe"
    name @1 :Text;        # e.g. "fs.read", "createInvoice"
    version @2 :Text;     # semver
}

struct ToolInfo {
    id @0 :ToolId;
    description @1 :Text;
    effect @2 :Effect;
    idempotent @3 :Bool;
    inputSchema @4 :Data;     # Cap'n Proto schema hash or JSON Schema
    outputSchema @5 :Data;
    provider @6 :Text;        # Which node provides this
    stability @7 :Stability;
}

enum Stability {
    experimental @0;
    beta @1;
    stable @2;
    deprecated @3;
}

struct CatalogSnapshot {
    version @0 :UInt64;
    timestamp @1 :UInt64;
    tools @2 :List(ToolInfo);
    hash @3 :Data;
    certificate @4 :Certificate;
}

struct CatalogDelta {
    version @0 :UInt64;
    added @1 :List(ToolInfo);
    removed @2 :List(ToolId);
    updated @3 :List(ToolInfo);
}

interface Catalog {
    # List all tools (optionally certified only)
    listTools @0 (certifiedOnly :Bool, ctx :CallContext) -> (tools :List(ToolInfo))
        $effect(deterministic)
        $idempotent(true);

    # Get tool by ID
    getTool @1 (id :ToolId, ctx :CallContext) -> (tool :ToolInfo)
        $effect(deterministic)
        $idempotent(true);

    # Search tools
    search @2 (query :Text, ctx :CallContext) -> (tools :List(ToolInfo))
        $effect(deterministic)
        $idempotent(true);

    # Get current snapshot
    snapshot @3 (ctx :CallContext) -> (snapshot :CatalogSnapshot)
        $effect(deterministic)
        $idempotent(true);

    # Subscribe to catalog changes
    subscribe @4 () -> (stream :EventStream)
        $effect(nondeterministic);

    # Invoke any tool by ID (universal gateway)
    invoke @5 (id :ToolId, args :Data, ctx :CallContext) -> (result :Data)
        $effect(nondeterministic);
}

# ============================================================================
# COORDINATION: Mesh Consensus
# LLM Committee, Provider Selection
# ============================================================================

struct Certificate {
    topic @0 :Data;
    proposalHash @1 :Data;
    round @2 :UInt32;
    confidence @3 :Float64;
    attestors @4 :List(Attestor);
    timestamp @5 :UInt64;
}

struct Attestor {
    nodeId @0 :Text;
    signature @1 :Data;
    publicKey @2 :Data;
}

struct ConsensusConfig {
    rounds @0 :UInt32;
    k @1 :UInt32;              # Sample size per round
    alpha @2 :Float64;         # Confidence threshold
    beta1 @3 :Float64;         # Phase I threshold
    beta2 @4 :Float64;         # Phase II (finality) threshold
    timeoutMs @5 :UInt64;
}

struct ConsensusVote {
    round @0 :UInt32;
    peerId @1 :Text;
    vote @2 :Data;
    confidence @3 :Float64;
    luminance @4 :Float64;
    signature @5 :Data;
    timestamp @6 :UInt64;
}

struct ConsensusResult {
    winner @0 :Data;
    synthesis @1 :Text;
    confidence @2 :Float64;
    round @3 :UInt32;
    votes @4 :List(ConsensusVote);
    certificate @5 :Certificate;
    durationNs @6 :UInt64;
}

interface Coordination {
    # Propose a consensus topic
    propose @0 (topic :Data, proposal :Data, config :ConsensusConfig, ctx :CallContext) -> (result :ConsensusResult)
        $effect(nondeterministic);

    # Sample peers for voting
    sample @1 (roundId :Text, k :UInt32, ctx :CallContext) -> (peers :List(Text))
        $effect(nondeterministic);

    # Cast a vote
    vote @2 (roundId :Text, vote :Data, confidence :Float64, ctx :CallContext) -> (accepted :Bool)
        $effect(nondeterministic);

    # Get current preference
    preference @3 (roundId :Text, ctx :CallContext) -> (winner :Data, confidence :Float64)
        $effect(deterministic);

    # Finalize consensus
    finalize @4 (roundId :Text, ctx :CallContext) -> (certificate :Certificate)
        $effect(nondeterministic);

    # LLM Committee: ask question, get certified answer
    committee @5 (question :Text, participants :List(Text), config :ConsensusConfig, ctx :CallContext) -> (answer :Text, certificate :Certificate)
        $effect(nondeterministic);
}

# ============================================================================
# MESH: Peer Discovery
# ============================================================================

struct PeerInfo {
    id @0 :Text;
    endpoint @1 :Text;
    capabilities @2 :EndpointCaps;
    publicKey @3 :Data;
    tools @4 :List(ToolId);
    load @5 :Float64;
    latencyMs @6 :UInt32;
}

struct MeshTopology {
    peers @0 :List(PeerInfo);
    version @1 :UInt64;
    timestamp @2 :UInt64;
}

interface Mesh {
    register @0 (info :PeerInfo, ctx :CallContext) -> (accepted :Bool, reason :Text)
        $effect(nondeterministic);

    deregister @1 (ctx :CallContext) -> ()
        $effect(nondeterministic);

    topology @2 (ctx :CallContext) -> (mesh :MeshTopology)
        $effect(deterministic);

    subscribe @3 () -> (stream :EventStream)
        $effect(nondeterministic);

    ping @4 (peerId :Text, ctx :CallContext) -> (latencyNs :UInt64)
        $effect(nondeterministic);
}

# ============================================================================
# MCP GATEWAY: Legacy Bridge
# ============================================================================

struct McpToolSchema {
    jsonSchema @0 :Text;
    schemaHash @1 :Data;
}

struct McpTool {
    name @0 :Text;
    description @1 :Text;
    inputSchema @2 :McpToolSchema;
}

interface Gateway {
    # List MCP tools from connected servers
    listMcpTools @0 (ctx :CallContext) -> (tools :List(McpTool))
        $effect(deterministic);

    # Call MCP tool
    callMcpTool @1 (name :Text, jsonArgs :Text, ctx :CallContext) -> (jsonResult :Text)
        $effect(nondeterministic);

    # Convert formats
    zapToMcp @2 (content :List(ContentBlock)) -> (json :Text)
        $effect(pure);

    mcpToZap @3 (json :Text) -> (content :List(ContentBlock))
        $effect(pure);

    # Register MCP server
    registerMcpServer @4 (name :Text, endpoint :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);

    # Unregister MCP server
    unregisterMcpServer @5 (name :Text, ctx :CallContext) -> (success :Bool)
        $effect(nondeterministic);
}

# ============================================================================
# Resources & Prompts (MCP-adjacent)
# ============================================================================

struct Resource {
    uri @0 :Text;
    name @1 :Text;
    description @2 :Text;
    mimeType @3 :Text;
    size @4 :UInt64;
}

struct ResourceContent {
    uri @0 :Text;
    mimeType @1 :Text;
    union {
        text @2 :Text;
        blob @3 :Data;
        stream @4 :ByteStream;
    }
}

struct ResourceUpdate {
    uri @0 :Text;
    union {
        updated @1 :ResourceContent;
        deleted @2 :Void;
    }
    timestamp @3 :UInt64;
}

struct ResourcePage {
    resources @0 :List(Resource);
    nextCursor @1 :Cursor;
    hasMore @2 :Bool;
}

interface ResourceSubscription {
    next @0 () -> (update :ResourceUpdate, done :Bool);
    cancel @1 () -> ();
}

interface Resources {
    list @0 (cursor :Cursor, ctx :CallContext) -> (page :ResourcePage)
        $effect(deterministic)
        $idempotent(true);

    read @1 (uri :Text, ctx :CallContext) -> (content :ResourceContent)
        $effect(deterministic);

    subscribe @2 (uri :Text) -> (sub :ResourceSubscription)
        $effect(nondeterministic);
}

struct PromptArg {
    name @0 :Text;
    description @1 :Text;
    required @2 :Bool;
}

struct Prompt {
    name @0 :Text;
    description @1 :Text;
    arguments @2 :List(PromptArg);
}

struct PromptPage {
    prompts @0 :List(Prompt);
    nextCursor @1 :Cursor;
    hasMore @2 :Bool;
}

struct Message {
    role @0 :Text;
    content @1 :List(ContentBlock);
}

struct PromptInstance {
    description @0 :Text;
    messages @1 :List(Message);
}

interface Prompts {
    list @0 (cursor :Cursor, ctx :CallContext) -> (page :PromptPage)
        $effect(pure)
        $idempotent(true);

    get @1 (name :Text, args :Data, ctx :CallContext) -> (prompt :PromptInstance)
        $effect(pure);
}

# ============================================================================
# Client Callbacks
# ============================================================================

struct Root {
    uri @0 :Text;
    name @1 :Text;
}

struct SamplingRequest {
    messages @0 :List(Message);
    modelPreferences @1 :ModelPreferences;
    systemPrompt @2 :Text;
    maxTokens @3 :UInt64;
}

struct ModelPreferences {
    hints @0 :List(Text);
    costPriority @1 :Float64;
    speedPriority @2 :Float64;
    intelligencePriority @3 :Float64;
}

struct SamplingResult {
    role @0 :Text;
    content @1 :List(ContentBlock);
    model @2 :Text;
    stopReason @3 :Text;
}

interface ClientCallbacks {
    roots @0 () -> (roots :List(Root))
        $effect(deterministic);

    onRootsChanged @1 () -> ()
        $effect(nondeterministic);

    createMessage @2 (request :SamplingRequest) -> (result :SamplingResult)
        $effect(nondeterministic);

    elicit @3 (prompt :Text, schema :Data) -> (response :Data)
        $effect(nondeterministic);
}

# ============================================================================
# Logging
# ============================================================================

enum LogLevel {
    debug @0;
    info @1;
    notice @2;
    warning @3;
    error @4;
    critical @5;
}

struct LogEntry {
    level @0 :LogLevel;
    logger @1 :Text;
    message @2 :Text;
    data @3 :Data;
    timestamp @4 :UInt64;
    traceId @5 :Text;
}

interface Log {
    setLevel @0 (level :LogLevel) -> ()
        $effect(pure);

    subscribe @1 () -> (stream :EventStream)
        $effect(nondeterministic);
}

# ============================================================================
# Top-Level ZAP Endpoint Interface
# ============================================================================

interface Zap {
    # Bootstrap
    initialize @0 (hello :Hello) -> (welcome :Welcome)
        $effect(pure)
        $idempotent(true);

    # Canonical tools (drop-in Claude Code replacements)
    fs @1 () -> (fs :Fs) $effect(pure);
    proc @2 () -> (proc :Proc) $effect(pure);
    vcs @3 () -> (vcs :Vcs) $effect(pure);
    code @4 () -> (code :Code) $effect(pure);
    net @5 () -> (net :Net) $effect(pure);
    repl @6 () -> (repl :Repl) $effect(pure);
    notebook @7 () -> (notebook :Notebook) $effect(pure);
    browser @8 () -> (browser :Browser) $effect(pure);
    agent @9 () -> (agent :Agent) $effect(pure);
    user @10 () -> (user :User) $effect(pure);

    # MCP-adjacent
    resources @11 () -> (resources :Resources) $effect(pure);
    prompts @12 () -> (prompts :Prompts) $effect(pure);

    # Mesh
    catalog @13 () -> (catalog :Catalog) $effect(pure);
    coordination @14 () -> (coordination :Coordination) $effect(pure);
    mesh @15 () -> (mesh :Mesh) $effect(pure);
    gateway @16 () -> (gateway :Gateway) $effect(pure);

    # Operations
    log @17 () -> (log :Log) $effect(pure);
    setClient @18 (client :ClientCallbacks) -> () $effect(nondeterministic);

    # Health
    ping @19 () -> (latencyNs :UInt64, serverTime :UInt64)
        $effect(pure)
        $idempotent(true);
}
