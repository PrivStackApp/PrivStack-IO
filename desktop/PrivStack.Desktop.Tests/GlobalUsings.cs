// Maps PrivStack.Services namespaces into test scope
// so tests referencing types that moved from Desktop to Services continue to compile.
global using PrivStack.Services;
global using PrivStack.Services.Abstractions;
global using PrivStack.Services.AI;
global using PrivStack.Services.Api;
global using PrivStack.Services.Biometric;
global using PrivStack.Services.Connections;
global using PrivStack.Services.FileSync;
global using PrivStack.Services.Ipc;
global using PrivStack.Services.Models;
global using PrivStack.Services.Models.PluginRegistry;
global using PrivStack.Services.Native;
global using PrivStack.Services.Plugin;
global using PrivStack.Services.Sdk;
global using PrivStack.Services.Update;
