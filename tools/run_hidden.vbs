' Launch a PowerShell script with no console window at all. Scheduled tasks
' running powershell.exe directly flash a window on every fire, which is
' exactly what an always-on exhibition supervisor must not do.
'   wscript //B //Nologo run_hidden.vbs <script.ps1> [args...]
Dim sh, cmd, i
Set sh = CreateObject("WScript.Shell")
cmd = "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File"
For i = 0 To WScript.Arguments.Count - 1
    cmd = cmd & " """ & WScript.Arguments(i) & """"
Next
sh.Run cmd, 0, False
