; Inno Setup >= 6.3; build through tools/workspace.py.
; PayloadDir is an exact allowlisted staging directory, never assets/ or target/.
#ifndef PayloadDir
  #error PayloadDir must identify the prepared installation payload
#endif
#ifndef AppVersion
  #error AppVersion must be a numeric x.y.z version
#endif
#ifndef AppArch
  #error AppArch must be x64 or arm64
#endif
#ifndef OutputRoot
  #error OutputRoot must identify the installer output directory
#endif
#ifndef CatalogFilename
  #error CatalogFilename must come from the resource installation policy
#endif
#ifndef CacheRelative
  #error CacheRelative must come from the resource installation policy
#endif

[Setup]
AppId=RazerUI-FD42CC53-D87A-421A-A65F-823EB9F9C144
AppName=Razer UI
AppVersion={#AppVersion}
AppPublisher=Razer UI contributors
VersionInfoCompany=Razer UI contributors
VersionInfoDescription=Razer UI installer
DefaultDirName={localappdata}\Programs\Razer UI
DefaultGroupName=Razer UI
PrivilegesRequired=lowest
#if AppArch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
; The locked GPUI build imports the Windows ICU C API (Windows 10 1703+).
MinVersion=10.0.15063
DisableProgramGroupPage=yes
DisableDirPage=auto
UsePreviousAppDir=yes
SetupIconFile={#PayloadDir}\Razer UI.ico
UninstallDisplayIcon={app}\Razer UI.ico
OutputDir={#OutputRoot}
OutputBaseFilename=Razer-UI-{#AppVersion}-{#AppArch}-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
Uninstallable=yes
UninstallDisplayName=Razer UI
; No launch-on-finish: installing never starts device or service operations.

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
#if AppArch == "x64"
Name: "hardwaremonitor"; Description: "Enable local CPU and GPU hardware monitoring (requires administrator approval)"
#endif

[Files]
Source: "{#PayloadDir}\razer_ui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\Razer UI.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\razer-ui-installation.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\{#CatalogFilename}"; DestDir: "{app}"; Flags: ignoreversion
; This generated include enumerates explicitly supplied, verified runtime DLLs.
#include AddBackslash(PayloadDir) + "..\runtime-files.iss"

[Dirs]
; Caches survive upgrades. Only the scoped uninstall code may delete the cache.
Name: "{app}\{#CacheRelative}"; Flags: uninsneveruninstall

[Icons]
Name: "{userprograms}\Razer UI"; Filename: "{app}\razer_ui.exe"; WorkingDir: "{app}"; IconFilename: "{app}\Razer UI.ico"
Name: "{userdesktop}\Razer UI"; Filename: "{app}\razer_ui.exe"; WorkingDir: "{app}"; IconFilename: "{app}\Razer UI.ico"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Razer UI"; ValueType: string; ValueName: "InstallLocation"; ValueData: "{app}"; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Razer UI"; ValueType: string; ValueName: "CacheRelative"; ValueData: "{#CacheRelative}"; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Razer UI"; ValueType: string; ValueName: "Version"; ValueData: "{#AppVersion}"; Flags: uninsdeletevalue uninsdeletekeyifempty

[Code]
var
  RemoveCachedAssets: Boolean;
  OwnsInstallation: Boolean;
  OwnedInstallDirectory: String;

function PlainDirectoryOrMissing(const DirectoryName: String): Boolean;
var
  FindRec: TFindRec;
begin
  Result := True;
  if FindFirst(DirectoryName, FindRec) then
  begin
    Result := ((FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0) and
      ((FindRec.Attributes and FILE_ATTRIBUTE_REPARSE_POINT) = 0);
    FindClose(FindRec);
  end;
end;

function SafeDirectoryChain(const DirectoryName: String): Boolean;
var
  CurrentDirectory, ParentDirectory: String;
begin
  CurrentDirectory := RemoveBackslashUnlessRoot(ExpandFileName(DirectoryName));
  Result := True;
  repeat
    if not PlainDirectoryOrMissing(CurrentDirectory) then
    begin
      Result := False;
      Exit;
    end;
    ParentDirectory := RemoveBackslashUnlessRoot(ExtractFileDir(CurrentDirectory));
    if CompareText(CurrentDirectory, ParentDirectory) = 0 then
      Exit;
    CurrentDirectory := ParentDirectory;
  until CurrentDirectory = '';
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if not SafeDirectoryChain(ExpandConstant('{app}\{#CacheRelative}')) then
    Result := 'The installation or resource-cache directory contains a link or a file. Choose an ordinary writable directory.';
end;

function RunHardwareDriverAction(const Parameters: String): Boolean;
var
  ResultCode: Integer;
begin
  Result := ShellExec('runas', ExpandConstant('{app}\razer_ui.exe'), Parameters,
    ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if Result then
    Result := ResultCode = 0;
  if not Result then
    Log('Hardware monitoring installation command failed or was cancelled. Code: ' + IntToStr(ResultCode));
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  DriverInstalled: Cardinal;
begin
#if AppArch == "x64"
  if CurStep <> ssPostInstall then
    Exit;
  if WizardIsTaskSelected('hardwaremonitor') then
  begin
    if not RegWriteDWordValue(HKCU, 'Software\Razer UI', 'HardwareMonitorInstalled', 2) then
      RaiseException('Hardware monitoring setup could not register its pending cleanup.');
    if not RunHardwareDriverAction('--install-hardware-driver') then
      RaiseException('Hardware monitoring could not be installed. No successful hardware setup has been recorded.');
    if not RegWriteDWordValue(HKCU, 'Software\Razer UI', 'HardwareMonitorInstalled', 1) then
      RaiseException('Hardware monitoring was installed, but its uninstall registration could not be saved.');
  end
  else if RegQueryDWordValue(HKCU, 'Software\Razer UI', 'HardwareMonitorInstalled', DriverInstalled)
    and (DriverInstalled <> 0) then
  begin
    if not RunHardwareDriverAction('--uninstall-hardware-driver') then
      RaiseException('Hardware monitoring could not be removed. Its installation registration has been kept.');
    RegDeleteValue(HKCU, 'Software\Razer UI', 'HardwareMonitorInstalled');
  end;
#endif
end;

function InitializeUninstall(): Boolean;
var
  RegisteredDirectory, KeepCache: String;
  Choice: Integer;
begin
  OwnedInstallDirectory := RemoveBackslashUnlessRoot(ExpandFileName(ExpandConstant('{app}')));
  OwnsInstallation := RegQueryStringValue(HKCU, 'Software\Razer UI',
    'InstallLocation', RegisteredDirectory);
  if OwnsInstallation then
    OwnsInstallation := CompareText(OwnedInstallDirectory,
      RemoveBackslashUnlessRoot(ExpandFileName(RegisteredDirectory))) = 0;

  Result := True;
  RemoveCachedAssets := False;
  KeepCache := ExpandConstant('{param:KEEPASSETCACHE|}');
  if KeepCache = '0' then
    RemoveCachedAssets := True
  else if (KeepCache <> '') and (KeepCache <> '1') then
  begin
    Log('KEEPASSETCACHE must be 0 or 1. Uninstall cancelled.');
    Result := False;
  end
  else if (KeepCache = '') and not UninstallSilent then
  begin
    Choice := MsgBox('Delete downloaded product resources from this installation?'
      + #13#10 + #13#10 + 'Yes: remove the resource cache.'
      + #13#10 + 'No: keep it for reinstalling and offline reuse.'
      + #13#10 + 'Device profiles and user settings are kept in either case.',
      mbConfirmation, MB_YESNOCANCEL);
    if Choice = IDCANCEL then
      Result := False
    else
      RemoveCachedAssets := Choice = IDYES;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  CacheDirectory: String;
  DriverInstalled: Cardinal;
begin
  if (CurUninstallStep = usUninstall) and OwnsInstallation then
  begin
    if RegQueryDWordValue(HKCU, 'Software\Razer UI', 'HardwareMonitorInstalled', DriverInstalled)
      and (DriverInstalled <> 0) then
    begin
      if not RunHardwareDriverAction('--uninstall-hardware-driver') then
      begin
        MsgBox('Hardware monitoring could not be removed. Close applications using it and retry uninstalling. The application files have been kept.', mbError, MB_OK);
        Abort;
      end;
      RegDeleteValue(HKCU, 'Software\Razer UI', 'HardwareMonitorInstalled');
    end;
  end;
  if (CurUninstallStep <> usPostUninstall) or not RemoveCachedAssets then
    Exit;
  CacheDirectory := AddBackslash(OwnedInstallDirectory) + '{#CacheRelative}';
  if not OwnsInstallation or not SafeDirectoryChain(CacheDirectory) then
  begin
    Log('Cache cleanup skipped: installation ownership or plain-directory checks failed.');
    Exit;
  end;
  // DelTree never follows reparse points within the cache (Inno Setup contract).
  if DirExists(CacheDirectory) and not DelTree(CacheDirectory, True, True, True) then
  begin
    Log('Some resource-cache files could not be removed: ' + CacheDirectory);
    if not UninstallSilent then
      MsgBox('Some downloaded resources could not be removed. Close applications using them, then remove this directory manually:'
        + #13#10 + CacheDirectory, mbInformation, MB_OK);
  end;
  RemoveDir(ExtractFileDir(CacheDirectory));
  RemoveDir(OwnedInstallDirectory);
end;
