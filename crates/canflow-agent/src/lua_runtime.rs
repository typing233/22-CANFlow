use canflow_types::{CanFlowError, CanFrame, CanId};
use mlua::prelude::*;

pub struct LuaRuntime {
    lua: Lua,
    script: String,
    name: String,
}

impl LuaRuntime {
    pub fn new(name: &str, script: &str, sandboxed: bool) -> Result<Self, CanFlowError> {
        let lua = Lua::new();

        if sandboxed {
            lua.globals().set("os", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
            lua.globals().set("io", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
            lua.globals().set("loadfile", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
            lua.globals().set("dofile", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
            lua.globals().set("require", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
            lua.globals().set("rawset", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
            lua.globals().set("rawget", LuaNil).map_err(|e| CanFlowError::Config(e.to_string()))?;
        }

        // Register CAN frame helper functions
        let can_frame_fn = lua.create_function(|_, (id, data): (u32, Vec<u8>)| {
            Ok(format!("frame:{}:{}", id, data.iter().map(|b| format!("{:02X}", b)).collect::<String>()))
        }).map_err(|e| CanFlowError::Config(e.to_string()))?;
        lua.globals().set("can_frame", can_frame_fn).map_err(|e| CanFlowError::Config(e.to_string()))?;

        // Random data generator for fuzzing
        let random_bytes_fn = lua.create_function(|_, len: usize| {
            let bytes: Vec<u8> = (0..len.min(8)).map(|_| rand::random::<u8>()).collect();
            Ok(bytes)
        }).map_err(|e| CanFlowError::Config(e.to_string()))?;
        lua.globals().set("random_bytes", random_bytes_fn).map_err(|e| CanFlowError::Config(e.to_string()))?;

        Ok(Self {
            lua,
            script: script.to_string(),
            name: name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn execute(&self) -> Result<Vec<CanFrame>, CanFlowError> {
        let result: LuaResult<LuaMultiValue> = self.lua.load(&self.script).eval();

        match result {
            Ok(values) => {
                let mut frames = Vec::new();
                for val in values {
                    if let LuaValue::String(s) = val {
                        if let Ok(str_val) = s.to_str() {
                            if let Some(frame) = parse_lua_frame(&str_val) {
                                frames.push(frame);
                            }
                        }
                    }
                }
                Ok(frames)
            }
            Err(e) => Err(CanFlowError::Config(format!("lua error: {}", e))),
        }
    }

    pub fn call_on_frame(&self, frame: &CanFrame) -> Result<Vec<String>, CanFlowError> {
        let globals = self.lua.globals();
        let on_frame: LuaResult<LuaFunction> = globals.get("on_frame");

        if let Ok(func) = on_frame {
            let table = self.lua.create_table().map_err(|e| CanFlowError::Config(e.to_string()))?;
            table.set("id", frame.id.raw_id()).map_err(|e| CanFlowError::Config(e.to_string()))?;
            table.set("dlc", frame.dlc).map_err(|e| CanFlowError::Config(e.to_string()))?;
            let data_vec: Vec<u8> = frame.payload().to_vec();
            table.set("data", data_vec).map_err(|e| CanFlowError::Config(e.to_string()))?;
            table.set("timestamp", frame.timestamp_ns).map_err(|e| CanFlowError::Config(e.to_string()))?;

            let result: LuaResult<LuaValue> = func.call(table);
            match result {
                Ok(LuaValue::String(s)) => {
                    Ok(vec![s.to_str().map(|v| v.to_string()).unwrap_or_default()])
                }
                Ok(LuaValue::Table(t)) => {
                    let mut results = Vec::new();
                    for pair in t.sequence_values::<String>() {
                        if let Ok(s) = pair {
                            results.push(s);
                        }
                    }
                    Ok(results)
                }
                Ok(_) => Ok(Vec::new()),
                Err(e) => Err(CanFlowError::Config(format!("lua on_frame error: {}", e))),
            }
        } else {
            Ok(Vec::new())
        }
    }
}

fn parse_lua_frame(s: &str) -> Option<CanFrame> {
    let parts: Vec<&str> = s.strip_prefix("frame:")?.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let id: u32 = parts[0].parse().ok()?;
    let data: Vec<u8> = (0..parts[1].len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&parts[1][i..i + 2], 16).ok())
        .collect();

    let can_id = if id > 0x7FF {
        CanId::extended(id)
    } else {
        CanId::standard(id as u16)
    };

    Some(CanFrame::new(can_id, &data))
}
