import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { Bell, Check, ChevronRight, Clock3, Egg, Folder, ImagePlus, Maximize2, Minus, MonitorCog, Palette, Plus, RefreshCw, RotateCcw, Settings, Sparkles, Trash2, Upload, X } from "lucide-react";
import type { AppSettings, EasterPreview, IconRule, TargetKind } from "./types";

const DEFAULTS: AppSettings = { monitorEnabled: false, monitorIntervalMinutes: 10, autostart: false, rules: [] };
const isTauri = () => "__TAURI_INTERNALS__" in window;

const STYLES = [
  { id: "mac-light", name: "macOS 浅色", os: "mac" }, { id: "mac-dark", name: "macOS 深色", os: "mac" },
  { id: "glass", name: "玻璃质感", os: "mac" }, { id: "round", name: "圆润", os: "mac" },
  { id: "win11", name: "Windows 11", os: "windows" }, { id: "win10", name: "Windows 10", os: "windows" },
  { id: "classic", name: "经典", os: "windows" }, { id: "flat", name: "扁平", os: "windows" }
];
const FILE_STYLES = [
  { id: "document", name: "文档" }, { id: "soft-document", name: "圆角文档" },
  { id: "card", name: "卡片" }, { id: "square", name: "方形" }
];

function folderPath(color: string, style: string) {
  if (style === "win10" || style === "classic") return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<path fill="${color}" d="M28 132h190l35 42h231v252H28z"/>
<path fill="${color}" d="M48 190h440l-45 237H28z"/>
</svg>`;
  if (style === "flat") return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<path fill="${color}" d="M28 126h177l47 51h232v270H28z"/>
</svg>`;
  if (style === "win11") return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<path fill="${color}" d="M32 139c0-20 16-36 36-36h139l48 51h189c20 0 36 16 36 36v221c0 21-17 38-38 38H70c-21 0-38-17-38-38z"/>
</svg>`;
  const opacity = style === "mac-dark" ? ".78" : style === "glass" ? ".62" : "1";
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<path fill="${color}" opacity="${opacity}" d="M31 151c0-24 19-43 43-43h126l48 51h190c24 0 43 19 43 43v197c0 28-23 51-51 51H82c-28 0-51-23-51-51z"/>
</svg>`;
}

function filePath(color: string, style: string) {
  if(style==="card")return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<rect x="52" y="85" width="408" height="342" rx="34" fill="${color}"/>
</svg>`;
  if(style==="square")return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<rect x="72" y="72" width="368" height="368" rx="18" fill="${color}"/>
</svg>`;
  const radius=style==="soft-document"?42:14;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
<path fill="${color}" d="M104 48h202l102 102v314H104z" rx="${radius}"/>
<path fill="#fff" fill-opacity=".28" d="M306 48v102h102z"/>
</svg>`;
}

export default function App() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULTS);
  const [tab, setTab] = useState<"create" | "rules" | "easter" | "settings">("create");
  const [kind, setKind] = useState<TargetKind>("folder");
  const [platform, setPlatform] = useState<"macos" | "windows" | "other">("macos");
  const [target, setTarget] = useState("");
  const [targets, setTargets] = useState<string[]>([]);
  const [color, setColor] = useState("#7c5cff");
  const [image, setImage] = useState<string>();
  const [sourceImage, setSourceImage] = useState<string>();
  const [removeBg, setRemoveBg] = useState(true);
  const [folderStyle, setFolderStyle] = useState("mac-light");
  const [fileStyle, setFileStyle] = useState("document");
  const [backgroundEnabled, setBackgroundEnabled] = useState(true);
  const [imageEnabled, setImageEnabled] = useState(true);
  const [recursiveMode, setRecursiveMode] = useState<"none" | "depth" | "all">("none");
  const [maxDepth, setMaxDepth] = useState(2);
  const [scale, setScale] = useState(55);
  const [offsetX, setOffsetX] = useState(0);
  const [offsetY, setOffsetY] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [pendingDelete, setPendingDelete] = useState<IconRule>();
  const [restoreTarget, setRestoreTarget] = useState("");
  const [restoreKind, setRestoreKind] = useState<"folder"|"file">("folder");
  const [easterRoot,setEasterRoot]=useState("");
  const [easterMode,setEasterMode]=useState<"none"|"depth"|"all">("depth");
  const [easterDepth,setEasterDepth]=useState(2);
  const [easterImage,setEasterImage]=useState<string>();
  const [easterPreview,setEasterPreview]=useState<EasterPreview>();
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => { if (isTauri()) { const refresh=()=>invoke<AppSettings>("load_settings").then(setSettings).catch(showError); refresh(); window.addEventListener("focus",refresh); invoke<"macos"|"windows"|"other">("get_platform").then(p => { setPlatform(p); setKind("folder"); setFolderStyle(p === "windows" ? "win11" : "mac-light"); }); return()=>window.removeEventListener("focus",refresh); } }, []);
  useEffect(() => { drawPreview(); }, [color, image, scale, offsetX, offsetY, folderStyle, fileStyle, kind, backgroundEnabled, imageEnabled]);
  useEffect(() => { if (sourceImage) processTransparency(sourceImage, removeBg).then(setImage); }, [sourceImage, removeBg]);
  useEffect(()=>{if(tab==="rules"&&isTauri())invoke<AppSettings>("check_rules").then(setSettings).catch(showError)},[tab]);

  const showError = (e: unknown) => { setNotice(String(e)); setTimeout(() => setNotice(""), 4500); };
  const saveSettings = async (next: AppSettings) => {
    setSettings(next);
    if (isTauri()) await invoke("save_settings", { settings: next });
  };

  const drawPreview = () => {
    const canvas = canvasRef.current; if (!canvas) return;
    const ctx = canvas.getContext("2d")!; ctx.clearRect(0, 0, 512, 512);
    const drawImageLayer=()=>{if(image&&imageEnabled){const img=new Image();img.onload=()=>{const max=330*scale/100,ratio=Math.min(max/img.width,max/img.height),w=img.width*ratio,h=img.height*ratio;ctx.save();ctx.globalCompositeOperation="source-over";ctx.drawImage(img,256-w/2+offsetX,278-h/2+offsetY,w,h);ctx.restore()};img.src=image}};
    if(backgroundEnabled){const base=new Image();base.onload=()=>{ctx.drawImage(base,0,0,512,512);drawImageLayer()};base.src=`data:image/svg+xml;charset=utf-8,${encodeURIComponent(kind==="folder"?folderPath(color,folderStyle):filePath(color,fileStyle))}`}else drawImageLayer();
  };

  const chooseTarget = async () => {
    if (!isTauri()) { setNotice("浏览器预览模式无法打开系统选择器，请在 Tauri 应用中运行"); return; }
    const selected = await open(kind === "folder" ? { directory: true, multiple: false } : { directory: false, multiple: true });
    if (!selected) return;
    if (kind === "folder") setTarget(selected as string);
    else setTargets(Array.from(new Set(Array.isArray(selected) ? selected : [selected])));
  };
  const addExtensions = () => {
    const values = target.split(/[,，;；\s]+/).filter(Boolean).map(value => {
      const normalized = value.startsWith(".") ? value : `.${value}`;
      return normalized.toLowerCase();
    });
    if (!values.length) return;
    setTargets(current => Array.from(new Set([...current, ...values])));
    setTarget("");
  };
  const removeTarget = (value: string) => setTargets(current => current.filter(item => item !== value));
  const chooseRestoreTarget=async(kind:"folder"|"file")=>{if(!isTauri())return;const selected=await open(kind==="folder"?{directory:true,multiple:false}:{directory:false,multiple:false});if(selected){setRestoreKind(kind);setRestoreTarget(selected as string)}};
  const oneClickRestore=async()=>{if(!restoreTarget)return showError("请先选择需要还原的目标");try{setSettings(await invoke<AppSettings>("restore_target",{target:restoreTarget,kind:restoreKind}));setNotice("已还原系统默认图标");setRestoreTarget("")}catch(e){showError(e)}};
  const chooseEasterRoot=async()=>{if(!isTauri())return;const value=await open({directory:true,multiple:false});if(value){setEasterRoot(value as string);setEasterPreview(undefined)}};
  const loadEasterImage=(file?:File)=>{if(!file)return;const reader=new FileReader();reader.onload=async()=>setEasterImage(await processTransparency(String(reader.result),true));reader.readAsDataURL(file)};
  const scanEaster=async()=>{if(!easterRoot)return showError("请先选择根目录");try{setEasterPreview(await invoke<EasterPreview>("preview_easter",{root:easterRoot,recursiveMode:easterMode,maxDepth:easterDepth}))}catch(e){showError(e)}};
  const applyEaster=async()=>{if(!easterPreview)return showError("请先扫描影响范围");if(!easterImage)return showError("请上传彩蛋图标");const ext=easterPreview.extensions.length?`\nWindows 将全局影响 ${easterPreview.extensions.length} 种扩展名。`:"";if(!window.confirm(`即将修改 ${easterPreview.directoryCount} 个目录和 ${easterPreview.fileCount} 个文件。${ext}\n确定继续吗？`))return;setBusy(true);try{setSettings(await invoke<AppSettings>("apply_easter",{root:easterRoot,recursiveMode:easterMode,maxDepth:easterDepth,pngData:easterImage.split(",")[1]}));setNotice("彩蛋模式已应用，可在本页整批还原");setEasterPreview(undefined)}catch(e){showError(e)}finally{setBusy(false)}};
  const restoreEaster=async(id:string)=>{if(!window.confirm("确定整批还原所有受影响的图标吗？"))return;try{setSettings(await invoke<AppSettings>("restore_easter",{id}));setNotice("彩蛋批次已全部还原")}catch(e){showError(e)}};
  const upload = (file?: File) => { if (!file) return; const reader = new FileReader(); reader.onload = () => setSourceImage(String(reader.result)); reader.readAsDataURL(file); };
  const processTransparency = (src: string, clean: boolean) => new Promise<string>((resolve) => { const img = new Image(); img.onload = () => { const ratio=Math.min(1,1000/Math.max(img.width,img.height)); const c=document.createElement("canvas");c.width=Math.round(img.width*ratio);c.height=Math.round(img.height*ratio);const x=c.getContext("2d",{willReadFrequently:true})!;x.drawImage(img,0,0,c.width,c.height);if(clean){const d=x.getImageData(0,0,c.width,c.height),p=d.data,w=c.width,h=c.height,seen=new Uint8Array(w*h),q=new Int32Array(w*h);let head=0,tail=0;const candidate=(i:number)=>p[i*4+3]<20||(Math.max(p[i*4],p[i*4+1],p[i*4+2])-Math.min(p[i*4],p[i*4+1],p[i*4+2])<22&&(p[i*4]+p[i*4+1]+p[i*4+2])/3>165);const push=(i:number)=>{if(!seen[i]&&candidate(i)){seen[i]=1;q[tail++]=i}};for(let xx=0;xx<w;xx++){push(xx);push((h-1)*w+xx)}for(let yy=0;yy<h;yy++){push(yy*w);push(yy*w+w-1)}while(head<tail){const i=q[head++],xx=i%w,yy=(i/w)|0;p[i*4+3]=0;if(xx)push(i-1);if(xx<w-1)push(i+1);if(yy)push(i-w);if(yy<h-1)push(i+w)}x.putImageData(d,0,0)}resolve(c.toDataURL("image/png"));};img.src=src;});
  const apply = async () => {
    const pendingExtensions = kind === "extension" ? target.split(/[,，;；\s]+/).filter(Boolean).map(value => (value.startsWith(".") ? value : `.${value}`).toLowerCase()) : [];
    const applyTargets = kind === "folder" ? (target.trim() ? [target.trim()] : []) : Array.from(new Set([...targets, ...pendingExtensions]));
    if (!applyTargets.length) return showError(kind === "extension" ? "请添加至少一个扩展名" : "请先选择目标");
    setBusy(true);
    try {
      const pngData = canvasRef.current!.toDataURL("image/png").split(",")[1];
      if (!isTauri()) throw new Error("当前是浏览器预览模式，请使用桌面应用应用图标");
      const failures: string[] = [];
      for (const item of applyTargets) {
        const name = kind === "extension" ? `${item} 文件` : (item.split(/[\\/]/).pop() || "文件");
        try {
          await invoke<IconRule>("apply_icon", { target: item, kind, pngData, name, addToMonitor: true, recursiveMode: kind === "folder" ? recursiveMode : "none", maxDepth });
        } catch (error) {
          failures.push(`${name}：${String(error)}`);
        }
      }
      setSettings(await invoke<AppSettings>("load_settings"));
      if (failures.length) throw new Error(`已成功 ${applyTargets.length - failures.length} 项，失败 ${failures.length} 项：${failures.join("；")}`);
      setTargets(kind === "folder" ? targets : applyTargets);
      setTarget(kind === "extension" ? "" : target);
      setNotice(`已为 ${applyTargets.length} 个目标应用图标并创建保护规则`);
    } catch (e) { showError(e); } finally { setBusy(false); }
  };
  const displayName = useMemo(() => kind === "folder" ? (target.split(/[\\/]/).pop() || "新目录") : targets.length ? `已选 ${targets.length} 项` : kind === "extension" ? "文件类型" : "文件", [target, targets, kind]);
  const updateMonitor = async (enabled: boolean) => { if (isTauri()) setSettings(await invoke<AppSettings>("configure_monitor", { enabled, intervalMinutes: settings.monitorIntervalMinutes })); else setSettings({...settings,monitorEnabled:enabled}); };
  const repair = async () => { setBusy(true); try { const count = isTauri() ? await invoke<number>("repair_all") : 0; setNotice(`修复完成，共检查 ${count} 条规则`); } catch(e) { showError(e); } finally { setBusy(false); } };
  const removeRule = async (rule: IconRule, restore: boolean) => { try { if(isTauri()) setSettings(await invoke<AppSettings>("delete_rule",{id:rule.id,restore})); else setSettings({...settings,rules:settings.rules.filter(r=>r.id!==rule.id)}); } catch(e) { showError(e); } };
  const toggleRule = async (rule: IconRule, enabled: boolean) => { try { if(isTauri()) setSettings(await invoke<AppSettings>("toggle_rule",{id:rule.id,enabled})); } catch(e){showError(e)} };
  const kinds: TargetKind[] = platform === "windows" ? ["folder","extension"] : platform === "macos" ? ["folder","file"] : ["folder"];
  const visibleStyles = STYLES.filter(s => s.os === (platform === "windows" ? "windows" : "mac"));

  const windowAction=(action:"minimize"|"maximize"|"close")=>{if(!isTauri())return;const win=getCurrentWindow();if(action==="minimize")win.minimize();else if(action==="maximize")win.toggleMaximize();else win.close()};
  return <div className="window-root">
    <div className="titlebar" data-tauri-drag-region onDoubleClick={()=>windowAction("maximize")}><span data-tauri-drag-region>IconCraft 图标工坊</span><div><button title="最小化" onClick={()=>windowAction("minimize")}><Minus/></button><button title="最大化或还原" onClick={()=>windowAction("maximize")}><Maximize2/></button><button className="close" title="关闭" onClick={()=>windowAction("close")}><X/></button></div></div>
  <div className="app-shell">
    <aside>
      <div className="brand">
<span>
<Sparkles size={19}/>
</span>
<div>IconCraft<small>图标工坊</small>
</div>
</div>
      <nav>
        <button className={tab === "create" ? "active" : ""} onClick={() => setTab("create")}>
<Palette/>图标设计</button>
        <button className={tab === "rules" ? "active" : ""} onClick={() => setTab("rules")}>
<MonitorCog/>规则 <em>{settings.rules.length}</em>
</button>
        <button className={tab === "easter" ? "active" : ""} onClick={() => setTab("easter")}><Egg/>彩蛋模式 <em>{settings.easterBatches?.length||0}</em></button>
        <button className={tab === "settings" ? "active" : ""} onClick={() => setTab("settings")}>
<Settings/>偏好设置</button>
      </nav>
      <div className="monitor-card">
<div>
<span className={settings.monitorEnabled ? "dot on" : "dot"}/>
<b>图标保护</b>
</div>
<small>{settings.monitorEnabled ? `每 ${settings.monitorIntervalMinutes} 分钟检查` : "图标保护已关闭"}</small>
<label className="switch">
<input type="checkbox" checked={settings.monitorEnabled} onChange={e => updateMonitor(e.target.checked)}/>
<i/>
</label>
</div>
      <div className="platform">适用于 Windows 与 macOS</div>
    </aside>
    <main>
      {tab === "create" && <>
        <header>
<div>
<p>图标设计</p>
<h1>给常用内容一点个性</h1>
<span>背景与自定义图片均可独立启用。</span>
</div>
<button className="secondary" onClick={() => {setImage(undefined);setSourceImage(undefined);setColor("#7c5cff");setScale(55);setOffsetX(0);setOffsetY(0);setBackgroundEnabled(true);setImageEnabled(true)}}>
<RotateCcw/>重置</button>
</header>
        <section className="designer">
          <div className="controls">
            <div className="step">
<i>1</i>
<div>
<b>应用到哪里？</b>
<span>选择目标类型与位置</span>
</div>
</div>
            <div className="segmented" style={{gridTemplateColumns:`repeat(${kinds.length},1fr)`}}>{kinds.map(v => <button key={v} className={kind === v ? "active" : ""} onClick={() => {setKind(v);setTarget("");setTargets([])}}>{v === "folder" ? "目录" : v === "extension" ? "文件类型" : "文件"}</button>)}</div>
            <div className="path-row">
<input value={kind === "file" ? (targets.length ? `已选择 ${targets.length} 个文件` : "") : target} readOnly={kind === "file"} onChange={e => setTarget(e.target.value)} onKeyDown={e=>{if(kind === "extension" && (e.key === "Enter" || e.key === ",")){e.preventDefault();addExtensions()}}} placeholder={kind === "extension" ? "输入扩展名，按回车添加，如 .md" : `选择${kind === "folder" ? "目录" : "文件"}路径`}/>{kind === "extension" ? <button onClick={addExtensions}><Plus/>添加</button> : <button onClick={chooseTarget}>
<Folder/>{kind === "file" ? "多选文件" : "选择"}</button>}</div>
            {kind !== "folder" && targets.length > 0 && <div className="target-chips">{targets.map(value=><span key={value} title={value}>{kind === "file" ? value.split(/[\\/]/).pop() : value}<button title="移除" onClick={()=>removeTarget(value)}><X/></button></span>)}</div>}
            {kind === "file" && <div className="hint">可一次选择多个文件，并为每个文件创建独立规则。</div>}
            {kind === "extension" && <div className="hint">可输入多个扩展名，用回车、空格或逗号分隔。</div>}
            {kind === "folder" && <div className="recursion">
<b>应用范围</b>
<div className="segmented">
<button className={recursiveMode==="none"?"active":""} onClick={()=>setRecursiveMode("none")}>仅当前目录</button>
<button className={recursiveMode==="depth"?"active":""} onClick={()=>setRecursiveMode("depth")}>限制层级</button>
<button className={recursiveMode==="all"?"active":""} onClick={()=>setRecursiveMode("all")}>全部递归</button>
</div>{recursiveMode==="depth"&&<label>递归层级 <input type="number" min="1" max="10" value={maxDepth} onChange={e=>setMaxDepth(Math.max(1,Math.min(10,+e.target.value)))}/>
</label>}</div>}
            <hr/>
            <div className="step layer-step">
<i>2</i>
<div>
<b>{kind==="folder"?"文件夹背景":"文件背景"}</b>
<span>仅保留简洁背景形状</span>
</div>
<label className="switch">
<input type="checkbox" checked={backgroundEnabled} onChange={e=>setBackgroundEnabled(e.target.checked)}/>
<i/>
</label>
</div>
            <div className={backgroundEnabled?"layer-content":"layer-content disabled"}>
<div className="colors">{["#7c5cff","#2f80ed","#13b981","#f2a33a","#f05252","#ec4899","#172033"].map(c => <button key={c} style={{background:c}} className={color === c ? "selected" : ""} onClick={() => setColor(c)}>{color === c && <Check/>}</button>)}<label>
<input type="color" value={color} onChange={e=>setColor(e.target.value)}/>
<Plus/>
</label>
</div>
            <div className="style-grid">{kind==="folder"?visibleStyles.map(s=>
<button key={s.id} className={folderStyle===s.id?"active":""} onClick={()=>setFolderStyle(s.id)}>
<span dangerouslySetInnerHTML={{__html:folderPath(color,s.id)}}/>{s.name}</button>):FILE_STYLES.map(s=>
<button key={s.id} className={fileStyle===s.id?"active":""} onClick={()=>setFileStyle(s.id)}>
<span dangerouslySetInnerHTML={{__html:filePath(color,s.id)}}/>{s.name}</button>)}</div>
</div>
            <hr/>
            <div className="step layer-step">
<i>3</i>
<div>
<b>叠加自定义图片</b>
<span>支持 PNG、JPG、WebP</span>
</div>
<label className="switch">
<input type="checkbox" checked={imageEnabled} onChange={e=>setImageEnabled(e.target.checked)}/>
<i/>
</label>
</div>
            <div className={imageEnabled?"layer-content":"layer-content disabled"}>
            <label className="drop" onDragOver={e=>e.preventDefault()} onDrop={e=>{e.preventDefault();upload(e.dataTransfer.files[0])}}>
<input type="file" accept="image/*" onChange={e=>upload(e.target.files?.[0])}/>
<Upload/>
<b>{image ? "更换图片" : "点击或拖拽上传图片"}</b>
<span>透明背景图片效果最佳</span>
</label>
            {sourceImage && <div className="image-actions">
<label className="check-row">
<input type="checkbox" checked={removeBg} onChange={e=>setRemoveBg(e.target.checked)}/>
<span>
<b>自动清除伪透明背景</b>
<small>处理棋盘格或与边缘相连的浅色背景</small>
</span>
</label>
<button onClick={()=>{setSourceImage(undefined);setImage(undefined)}}>
<Trash2/>清除图片</button>
</div>}
            {image && <div className="adjust">
<label>大小 <input type="range" min="20" max="120" value={scale} onChange={e=>setScale(+e.target.value)}/>
<span>{scale}%</span>
</label>
<label>水平 <input type="range" min="-100" max="100" value={offsetX} onChange={e=>setOffsetX(+e.target.value)}/>
</label>
<label>垂直 <input type="range" min="-100" max="100" value={offsetY} onChange={e=>setOffsetY(+e.target.value)}/>
</label>
</div>}
            </div>
          </div>
          <div className="preview-panel">
<div className="preview-title">
<div>
<b>实时预览</b>
<span>最终效果可能因系统略有不同</span>
</div>
<span className="badge">512 × 512</span>
</div>
<div className="preview-stage">
<canvas ref={canvasRef} width="512" height="512"/>
<span>{displayName}</span>
</div>
<button className="primary" disabled={busy} onClick={apply}>{busy ? <RefreshCw className="spin"/> : <Sparkles/>}{busy ? "正在处理…" : "应用此图标"}<ChevronRight/>
</button>
<p className="safe">
<Check/>应用后默认创建保护规则，可随时还原</p>
</div>
        </section>
      </>}
      {tab === "rules" && <>
<header>
<div>
<p>规则</p>
<h1>管理与还原图标</h1>
<span>每条规则可独立开启图标保护、删除或还原。</span>
</div>
<button className="secondary" onClick={repair}>
<RefreshCw/>立即检查</button>
</header>
<section className="quick-restore">
<div>
<RotateCcw/>
<span>
<b>一键还原</b>
<small>选择文件或目录，恢复系统默认图标</small>
</span>
</div>
<div className="restore-controls">
<input value={restoreTarget} readOnly placeholder="尚未选择目标"/>
<button onClick={()=>chooseRestoreTarget("folder")}><Folder/>选择目录</button>
{platform==="macos"&&<button onClick={()=>chooseRestoreTarget("file")}><ImagePlus/>选择文件</button>}
<button className="restore-action" onClick={oneClickRestore}>立即还原</button>
</div>
</section>
<section className="rules-card">{settings.rules.length === 0 ? <div className="empty">
<ImagePlus/>
<h3>还没有规则</h3>
<p>应用第一个图标后，可在这里还原。</p>
<button className="primary small" onClick={()=>setTab("create")}>开始设计</button>
</div> : settings.rules.map(rule => <div className="rule" key={rule.id}>
<img src={isTauri() ? `asset://localhost/${encodeURI(rule.iconPath)}` : ""}/>
<div>
<b>{rule.name}</b>
<span>{rule.target}{rule.recursiveMode&&rule.recursiveMode!=="none"?` · ${rule.recursiveMode==="all"?"全部递归":`递归 ${rule.maxDepth} 层`}`:""}</span>
</div>{rule.monitored!==false?<>
<label className="switch">
<input type="checkbox" disabled={rule.status==="目标已失效"||rule.status==="图标资源已丢失"} checked={rule.enabled} onChange={e=>toggleRule(rule,e.target.checked)}/>
<i/>
</label>
<span className={rule.enabled?"status":"status paused"}>
<i/>{rule.status==="目标已失效"?"目标已失效":rule.status==="图标资源已丢失"?"资源已丢失":rule.enabled?"保护中":"已关闭"}</span>
</>:<span className="status paused">
<i/>未保护</span>}<button title="删除规则" onClick={()=>setPendingDelete(rule)}>
<Trash2/>
</button>
<button className="restore-btn labeled" title="还原系统默认图标" onClick={()=>removeRule(rule,true)}>
<RotateCcw/>还原</button>
</div>)}</section>
</>}
      {tab === "easter" && <><header><div><p>彩蛋模式</p><h1>让所有图标变成同一个</h1><span>限定在所选根目录；Windows 文件扩展名会全局生效。</span></div></header><section className="easter-card"><div className="easter-form"><label><b>根目录</b><div className="path-row"><input value={easterRoot} readOnly placeholder="选择彩蛋模式作用范围"/><button onClick={chooseEasterRoot}><Folder/>选择</button></div></label><label><b>递归范围</b><div className="segmented"><button className={easterMode==="none"?"active":""} onClick={()=>{setEasterMode("none");setEasterPreview(undefined)}}>仅根目录</button><button className={easterMode==="depth"?"active":""} onClick={()=>{setEasterMode("depth");setEasterPreview(undefined)}}>限制层级</button><button className={easterMode==="all"?"active":""} onClick={()=>{setEasterMode("all");setEasterPreview(undefined)}}>全部递归</button></div>{easterMode==="depth"&&<input className="depth-input" type="number" min="1" max="10" value={easterDepth} onChange={e=>{setEasterDepth(Math.max(1,Math.min(10,+e.target.value)));setEasterPreview(undefined)}}/>}</label><label><b>统一图标</b><label className="drop"><input type="file" accept="image/png,image/jpeg,image/webp" onChange={e=>loadEasterImage(e.target.files?.[0])}/><Upload/><b>{easterImage?"更换彩蛋图标":"上传彩蛋图标"}</b></label></label><button className="secondary scan-btn" onClick={scanEaster}><RefreshCw/>扫描影响范围</button>{easterPreview&&<div className="impact"><b>扫描结果</b><span>{easterPreview.directoryCount} 个目录</span><span>{easterPreview.fileCount} 个文件</span>{platform==="windows"&&<><strong>全局影响 {easterPreview.extensions.length} 种扩展名</strong><small>{easterPreview.extensions.join("、")||"没有发现带扩展名的文件"}</small></>}</div>}<button className="primary" disabled={busy||!easterPreview||!easterImage} onClick={applyEaster}><Egg/>{busy?"正在批量应用…":"启动彩蛋模式"}</button></div><div className="batch-list"><h3>彩蛋批次</h3>{!settings.easterBatches?.length?<div className="empty compact"><Egg/><p>还没有彩蛋批次</p></div>:settings.easterBatches.map(batch=><div className="batch" key={batch.id}><img src={isTauri()?`asset://localhost/${encodeURI(batch.iconPath)}`:""}/><div><b>{batch.root.split(/[\\/]/).pop()}</b><span>{batch.directoryCount} 个目录 · {batch.fileCount} 个文件</span></div><button onClick={()=>restoreEaster(batch.id)}><RotateCcw/>整批还原</button></div>)}</div></section></>}
      {tab === "settings" && <>
<header>
<div>
<p>偏好设置</p>
<h1>让 IconCraft 按你的习惯工作</h1>
</div>
</header>
<section className="settings-card">
<div className="setting">
<span className="setting-icon">
<Clock3/>
</span>
<div>
<b>图标保护</b>
<p>定期检查已启用的规则，发现丢失后重新应用。</p>
</div>
<label className="switch">
<input type="checkbox" checked={settings.monitorEnabled} onChange={e=>updateMonitor(e.target.checked)}/>
<i/>
</label>
</div>
<div className="setting">
<span className="setting-icon">
<RefreshCw/>
</span>
<div>
<b>检查间隔</b>
<p>间隔越短，系统资源占用会略微增加。</p>
</div>
<select value={settings.monitorIntervalMinutes} onChange={async e=>{const n=+e.target.value; if(isTauri())setSettings(await invoke<AppSettings>("configure_monitor",{enabled:settings.monitorEnabled,intervalMinutes:n}))}}>
<option value="1">1 分钟</option>
<option value="5">5 分钟</option>
<option value="10">10 分钟</option>
<option value="30">30 分钟</option>
<option value="60">1 小时</option>
</select>
</div>
<div className="setting">
<span className="setting-icon">
<Bell/>
</span>
<div>
<b>托盘常驻</b>
<p>关闭主窗口后继续在系统托盘运行。</p>
</div>
<span className="pill">默认开启</span>
</div>
<div className="setting"><span className="setting-icon"><Sparkles/></span><div><b>开机自启</b><p>登录系统后自动启动 IconCraft 并在托盘运行。</p></div><label className="switch"><input type="checkbox" checked={settings.autostart} onChange={async e=>{try{setSettings(await invoke<AppSettings>("configure_autostart",{enabled:e.target.checked}))}catch(err){showError(err)}}}/><i/></label></div>
</section>
</>}
    </main>
    {pendingDelete&&<div className="modal-mask" onClick={()=>setPendingDelete(undefined)}>
<div className="confirm-modal" onClick={e=>e.stopPropagation()}>
<h3>删除“{pendingDelete.name}”规则？</h3>
<p>请选择是否同时还原系统默认图标。</p>
<div>
<button onClick={()=>setPendingDelete(undefined)}>取消</button>
<button onClick={async()=>{const r=pendingDelete;setPendingDelete(undefined);await removeRule(r,false)}}>仅删除规则</button>
<button className="danger" onClick={async()=>{const r=pendingDelete;setPendingDelete(undefined);await removeRule(r,true)}}>还原并删除</button>
</div>
</div>
</div>}
    {notice && <div className="toast">{notice}</div>}
  </div></div>;
}
