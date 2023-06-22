# Debug CoreDB Operator using Visual Studio

1. Follow steps from [this](https://code.visualstudio.com/docs/languages/rust) article to setup Rust with Visual Studio
2. Open `coredb-operator` folder in Visual Studio
3. Run `cargo build`
4. Start cluster locally using `kind` and make sure you switch to that context
5. Go to `Run` —> `Start Debugging` in Visual Studio which will prompt to Add a Launch Configuration
6. Add following configuration to `launch.json` file. See program section.

```json
{
	// Use IntelliSense to learn about possible attributes.
	// Hover to view descriptions of existing attributes.
	// For more information, visit: https://go.microsoft.com/fwlink/?linkid=830387
	"version": "0.2.0",
	"configurations": [
		{
			"type": "lldb",
			"request": "launch",
			"name": "Debug",
			"program": "${workspaceRoot}/target/debug/controller",
			"args": [],
			"cwd": "${workspaceFolder}"
		}
	]
}
```

1. Once launch.json file is added Start Debugging and you can be able to add a breakpoint and Debug the operator
2. Example change: Change a `CoreDB` custom resource in the local cluster manually and add `pgmq` extension and it should start the reconcile operation

Note: You can also use [rust-analyzer: Debug option](https://code.visualstudio.com/docs/languages/rust#_using-rust-analyzer-debug) but that only gives a way to run tests currently.
