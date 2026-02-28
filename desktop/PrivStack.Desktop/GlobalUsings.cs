// Global using directives — maps PrivStack.Services namespaces into Desktop scope
// so existing code that referenced PrivStack.Desktop.Services.* types continues to compile
// after the Services project extraction.

// Resolve Log ambiguity: PrivStack.Services.Log wraps Serilog.Log,
// so this alias lets all Desktop code use Log.ForContext<T>() etc. without conflict.
global using Log = PrivStack.Services.Log;
global using PrivStack.Services;
global using PrivStack.Services.Abstractions;
global using PrivStack.Services.AI;
global using PrivStack.Services.Api;
global using PrivStack.Services.Biometric;
global using PrivStack.Services.Connections;
global using PrivStack.Services.FileSync;
global using PrivStack.Services.Models;
global using PrivStack.Services.Models.PluginRegistry;
global using PrivStack.Services.Native;
global using PrivStack.Services.Plugin;
global using PrivStack.Services.Sdk;
global using PrivStack.Services.Update;
