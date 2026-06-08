-- Example Lua agent: periodic frame generator
-- Generates frames to keep an ECU session alive (TesterPresent)

local frames_sent = 0
local results = {}

-- Generate TesterPresent frames for multiple ECU addresses
local ecu_ids = {0x7E0, 0x7E1, 0x7E2, 0x7E3}

for _, ecu_id in ipairs(ecu_ids) do
    -- TesterPresent with suppressPositiveResponse
    local frame = can_frame(ecu_id, {0x02, 0x3E, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00})
    table.insert(results, frame)
    frames_sent = frames_sent + 1
end

-- Also send a DiagnosticSessionControl (extended session)
table.insert(results, can_frame(0x7DF, {0x02, 0x10, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00}))
frames_sent = frames_sent + 1

return table.unpack(results)
