-- Example Lua agent: on_frame callback for real-time analysis
-- Detects unexpected ECU reset requests

function on_frame(frame)
    -- Check for ECU Reset service (0x11)
    if frame.dlc >= 2 and frame.data[2] == 0x11 then
        local sub = frame.data[3] or 0
        local reset_type = "unknown"
        if sub == 0x01 then reset_type = "hardReset"
        elseif sub == 0x02 then reset_type = "keyOffOnReset"
        elseif sub == 0x03 then reset_type = "softReset"
        end

        return string.format("ALERT: ECU Reset detected on 0x%03X type=%s", frame.id, reset_type)
    end

    -- Check for SecurityAccess brute-force (rapid 0x27 requests)
    if frame.dlc >= 2 and frame.data[2] == 0x27 then
        return string.format("INFO: SecurityAccess on 0x%03X sub=0x%02X", frame.id, frame.data[3] or 0)
    end
end
