
"use strict";
/* ── helpers ──────────────────────────────────────────────────────────── */
const $=(s,r=document)=>r.querySelector(s), $$=(s,r=document)=>[...r.querySelectorAll(s)];
const root=document.documentElement, EASE="cubic-bezier(.2,0,0,1)";
const esc=s=>String(s??"").replace(/[&<>"']/g,c=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
const rub=k=>{const v=k/100;return (Number.isInteger(v)?v.toLocaleString("en").replace(/,/g," "):v.toFixed(2))+" ₽"};
const rate=b=>b<=0?"0":b<1024?b.toFixed(0)+" B/s":b<1048576?(b/1024).toFixed(0)+" KB/s":(b/1048576).toFixed(1)+" MB/s";
const clock=s=>[s/3600,s/60%60,s%60].map(n=>String(Math.floor(n)).padStart(2,"0")).join(":");
const until=d=>new Date(Date.now()+d*864e5).toLocaleDateString("en-GB",{day:"numeric",month:"long"});
const reduced=()=>matchMedia("(prefers-reduced-motion:reduce)").matches;
const I={
 globe:'<circle cx="10" cy="10" r="7"/><path d="M3 10h14M10 3c4 4 4 10 0 14M10 3c-4 4-4 10 0 14"/>',
  home:'<circle cx="10" cy="10" r="6.5"/>', power:'<path d="M10 3.5v6"/><path d="M6.2 5.8a6 6 0 1 0 7.6 0"/>',
  conns:'<path d="M6.5 16V4.5M3.6 7.4l2.9-2.9 2.9 2.9M13.5 4v11.5M10.6 12.6l2.9 2.9 2.9-2.9"/>',
  rules:'<circle cx="4.8" cy="15" r="2"/><circle cx="15.2" cy="5" r="2"/><path d="M6.8 15h5.4a2.5 2.5 0 0 0 0-5H7.8a2.5 2.5 0 0 1 0-5h5.4"/>',
  plans:'<path d="M3 7a1.5 1.5 0 0 1 1.5-1.5h11A1.5 1.5 0 0 1 17 7v1.5a1.5 1.5 0 0 0 0 3V13a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 3 13v-1.5a1.5 1.5 0 0 0 0-3z"/>',
  devices:'<path d="M10 13H3.5A1.5 1.5 0 0 1 2 11.5v-7A1.5 1.5 0 0 1 3.5 3h11A1.5 1.5 0 0 1 16 4.5V6M6 13v3M3 16h7"/><rect x="12" y="8" width="6" height="9" rx="1.3"/><path d="M14.5 14.5h1"/>',
  settings:'<path d="M3 6h8M15 6h2M13 4.5v3M3 14h2M9 14h8M7 12.5v3"/>',
  account:'<circle cx="10" cy="10" r="7.3"/><circle cx="10" cy="8.2" r="2.5"/><path d="M5.3 15.5c1-2.1 2.7-3.1 4.7-3.1s3.7 1 4.7 3.1"/>',
  check:'<path d="M4.5 10.5l3.5 3.5 7.5-8"/>', x:'<path d="M5.5 5.5l9 9M14.5 5.5l-9 9"/>', plus:'<path d="M10 4.5v11M4.5 10h11"/>',
  chev:'<path d="M8 5l5 5-5 5"/>', back:'<path d="M12 5l-5 5 5 5"/>', pause:'<path d="M7 5v10M13 5v10"/>', play:'<path d="M7 5l8 5-8 5z"/>',
  grip:'<circle cx="7.5" cy="6" r=".9"/><circle cx="12.5" cy="6" r=".9"/><circle cx="7.5" cy="10" r=".9"/><circle cx="12.5" cy="10" r=".9"/><circle cx="7.5" cy="14" r=".9"/><circle cx="12.5" cy="14" r=".9"/>',
  trash:'<path d="M4.5 6h11M8 6V4.5h4V6M6 6l.7 9.5h6.6L14 6"/>', warn:'<path d="M10 3.5l7 12.5H3zM10 8.5v3.5M10 14.2v.1"/>',
  ext:'<path d="M8 5H5v10h10v-3M11 4.5h4.5V9M15.5 4.5L9.5 10.5"/>', laptop:'<rect x="4.5" y="5" width="11" height="7.5" rx="1.2"/><path d="M2.5 15h15"/>', phone:'<rect x="6" y="3.5" width="8" height="13" rx="1.8"/><path d="M9 14.2h2"/>'
};
const ic=(n,c="")=>`<svg class="ic ${c}" viewBox="0 0 20 20" aria-hidden="true">${I[n]}</svg>`;

/* ── three minimal glass directions and three logo marks ─────────────── */
const VAR={
  1:{name:"Пыль",tag:"black only",themes:["dark"],word:"<b>акимов</b><i>vpn</i>",ctl:"dust",note:"Black, one warm accent. Dust orbits in a ring and settles into the logo when connected · Geologica"},
  2:{name:"Линза",tag:"dark + light",themes:["dark","light"],word:"<b>akimov</b><i>vpn</i>",ctl:"lens",note:"A liquid-glass lens over two drifting lights; colour arrives with the connection · Manrope"},
  3:{name:"Контур",tag:"dark + light",themes:["dark","light"],word:"<b>акимов</b><i>впн</i>",ctl:"ticks",note:"A ring of fine lines that moves with your real traffic; almost no colour · Jost"}
};
/* marks are strokes on a 64 grid: [path, width] and [cx, cy, r] */
const MARKS={
  1:{name:"Кров",why:"А without a crossbar: a roof with you under it.",p:[["M14 48L32 14L50 48",8]],c:[[32,43,5]]},
  2:{name:"Арка",why:"А with a round top: a gateway, a tunnel entrance.",p:[["M15 50V31a17 17 0 0 1 34 0V50",8],["M15 39H49",8]],c:[]},
  3:{name:"Буква",why:"Lowercase а, same in Cyrillic and Latin. Friendly, matches the lowercase name.",p:[["M44 34a14 14 0 1 1-28 0a14 14 0 0 1 28 0",8],["M46 19V49",8]],c:[]}
};
const markSVG=i=>`<svg class="mk" viewBox="0 0 64 64" aria-hidden="true" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">${MARKS[i].p.map(([d,w])=>`<path d="${d}" stroke-width="${w}"/>`).join("")}${MARKS[i].c.map(([x,y,r])=>`<circle cx="${x}" cy="${y}" r="${r}" fill="currentColor" stroke="none"/>`).join("")}</svg>`;
const CTL={
  dust:()=>`<canvas id="cv"></canvas>`,
  lens:()=>`<span class="l-blob a"></span><span class="l-blob b"></span><span class="l-glass"></span><span class="l-rim"></span><span class="l-mark">${markSVG(S.logo)}</span>`,
  ticks:()=>`<canvas id="cv"></canvas><span class="t-disc">${markSVG(S.logo)}</span>`
};

/* ── selected design and live account state ───────────────────────────── */
const TARIFFS=[];
const PERIODS=[{days:30,label:"1 mo",mult:1,mo:1},{days:90,label:"3 mo",mult:2.7,mo:3},{days:180,label:"6 mo",mult:5,mo:6},{days:365,label:"12 mo",mult:9,mo:12}];
const METHODS=[{id:"balance",name:"Balance",fee:0}], NODES=[];
const SYSTEM_RULES=[], PLAN_RULES=[];
const TYPES=[["DOMAIN-SUFFIX","Domain and subdomains","SUFFIX","example.com"],["DOMAIN","Exact domain","DOMAIN","cdn.example.com"],["DOMAIN-KEYWORD","Domain contains","KEYWORD","tracker"],["IP-CIDR","IP address or range","IP","203.0.113.0/24"],["PROCESS-NAME","App (process name)","PROCESS","steam.exe"],["DST-PORT","Port","PORT","25565"],["GEOSITE","Site category","GEOSITE","youtube"]];
const SHORT=Object.fromEntries(TYPES.map(t=>[t[0],t[2]]));
const TARGET={"@direct":["Direct","direct"],"@proxy":["Via VPN","proxy"],"@block":["Block","block"]};
const ORDER=["home","account","conns","rules","subs","balance","devices","referral","settings"];
Object.assign(I,{
  wallet:'<rect x="2.5" y="5" width="15" height="11" rx="2"/><path d="M3 5.5V4a1.5 1.5 0 0 1 1.5-1.5H15M17.5 8.5H13v4h4.5M14.8 10.5h.1"/>',
  gift:'<rect x="3.5" y="8.5" width="13" height="8" rx="1.5"/><path d="M2.8 5.8h14.4v2.7H2.8zM10 5.8v10.7M10 5.8C8.6 2.9 5.6 3.3 6 5.8M10 5.8c1.4-2.9 4.4-2.5 4 0"/>',
  tag:'<path d="M3.5 4.5v5.2l7.3 7.3a1.2 1.2 0 0 0 1.7 0l4.5-4.5a1.2 1.2 0 0 0 0-1.7L9.7 3.5H4.5a1 1 0 0 0-1 1z"/><circle cx="7" cy="7" r="1.1"/>',
  logout:'<path d="M8 4H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3M12.5 6.5L16 10l-3.5 3.5M16 10H8"/>',
  help:'<circle cx="10" cy="10" r="7.3"/><path d="M7.8 8a2.3 2.3 0 1 1 3.4 2c-.8.5-1.2 1-1.2 1.8M10 14.3v.1"/>',
  info:'<circle cx="10" cy="10" r="7.3"/><path d="M10 9.2v4.6M10 6.3v.1"/>'
});
/* a labelled section: one card, rows inside */
const group=(label,rows)=>`<section class="grp"><h2 class="over">${label}</h2><div class="card clip"><div class="list">${rows}</div></div></section>`;
const navrow=(v,i,t,d="",action="go")=>`<button class="set navrow" data-act="${action}" data-v="${v}">${ic(i)}<span class="grow"><span class="set-t">${t}</span>${d?`<span class="cap dim">${esc(d)}</span>`:""}</span>${ic("chev","s")}</button>`;
const inforow=(t,v)=>`<div class="set"><span class="grow set-t">${t}</span><span class="mono dim">${esc(v)}</span></div>`;

let uid=10;
const S={v:1,logo:3,screen:"home",conn:"idle",secs:0,mode:"tun",node:"",balance:0,activeSub:null,pendingSwitch:null,
  subs:[],devices:{},deviceLoaded:{},rules:[],conns:[],paused:false,filter:"all",q:"",scopeFor:null,applied:"Applied",
  comp:{type:"DOMAIN-SUFFIX",value:"",target:"@direct",raw:false},pay:null,sheet:null,edit:null,just:null,
  auth:{mode:"login",email:"",retryAt:0},authBusy:false,me:null,cabinet:null,has_sub:false,hwid:"",busy:false,version:"",signed_in:false,
  transactions:null,historyLoading:false,historyError:"",legal:null,legalLoading:false,legalError:""};
let historyGeneration=0;
const flagCodes=new Set(["EU","EE","LV","LT","FI","SE","PL","NL","DE","FR","GB","US","RU","TR","UA","KZ","CH","AT","BE","CZ","DK","NO","ES","IT","PT","IE","RO","CA","AU","JP","SG","HK","IN","GE","AM","IL","AE"]);
function nodeCountry(name){const pair=String(name||'').match(/[\u{1F1E6}-\u{1F1FF}]{2}/u);const code=pair?[...pair[0]].map(c=>String.fromCharCode(c.codePointAt(0)-0x1f1e6+65)).join(''):String(name||'').match(/^([A-Z]{2})\s/)?.[1];return flagCodes.has(code)?code:null}
function nodeLabel(name){const code=nodeCountry(name);return String(name||'').replace(/[\u{1F1E6}-\u{1F1FF}]{2}/gu,'').replace(code?new RegExp('^'+code+'\\s+'):/$^/,'').trim()}
function nodeFlag(name){const code=nodeCountry(name);return code?`<img class="flag" src="flags/${code.toLowerCase()}.svg" alt="${esc(new Intl.DisplayNames(['en'],{type:'region'}).of(code))} flag">`:ic('globe','s')}
const sub=()=>S.subs.find(s=>s.id===S.activeSub), nodeOf=id=>NODES.find(n=>n.id===id)||{id,cc:"··",name:nodeLabel(id)||"Automatic",sub:"From your subscription"};
const exitNode=()=>nodeOf(S.node);
const isMobile=()=>$("#win").offsetWidth<=720;

/* ── motion plumbing: thumbs, switches and numbers animate across re-renders ── */
const mem={seg:{},sw:{},n:{}};
const fmt={int:v=>Math.round(v),rub:v=>rub(Math.round(v))};
function tween(el,a,b){
  const f=fmt[el.dataset.f||"int"],d=620;if(reduced()){el.textContent=f(b);return}
  const t0=performance.now();el.textContent=f(a);
  const step=t=>{if(!el.isConnected)return;const k=Math.min(1,(t-t0)/d);el.textContent=f(a+(b-a)*(1-Math.pow(1-k,3)));if(k<1)requestAnimationFrame(step)};
  requestAnimationFrame(step);setTimeout(()=>{if(el.isConnected)el.textContent=f(b)},d+90);
}
function settle(scope=document){
  $$(".seg[data-k]",scope).forEach(el=>{const k=el.dataset.k,cur=+el.dataset.i,prev=mem.seg[k];if(prev!=null&&prev!==cur){el.style.setProperty("--i",prev);el.getBoundingClientRect();el.style.setProperty("--i",cur)}mem.seg[k]=cur});
  $$(".sw[data-k]",scope).forEach(el=>{const k=el.dataset.k,cur=el.getAttribute("aria-checked"),prev=mem.sw[k];if(prev!=null&&prev!==cur){el.setAttribute("aria-checked",prev);el.getBoundingClientRect();el.setAttribute("aria-checked",cur)}mem.sw[k]=cur});
  $$("[data-n]",scope).forEach(el=>{const k=el.dataset.nk,to=+el.dataset.n,from=mem.n[k]??0;mem.n[k]=to;if(from!==to)tween(el,from,to)});
}
const seg=(k,opts,val,act,cls="")=>{const i=Math.max(0,opts.findIndex(o=>o[0]===val));return`<div class="seg ${cls}" role="group" data-k="${k}" data-i="${i}" style="--n:${opts.length};--i:${i}">${opts.map(o=>`<button data-act="${act}" data-v="${o[0]}" aria-pressed="${o[0]===val}">${o[1]}</button>`).join("")}</div>`};
const num=(k,v,f="int")=>`<span data-n="${v}" data-nk="${k}" data-f="${f}">${fmt[f](v)}</span>`;
/* FLIP: rows glide to their new place instead of jumping */
function snap(){const m=new Map();$$("[data-flip]").forEach(c=>$$(":scope>[data-id]",c).forEach(e=>m.set(c.dataset.flip+":"+e.dataset.id,e.getBoundingClientRect())));return m}
function play(m){
  if(reduced())return;const had=new Set([...m.keys()].map(k=>k.split(":")[0]));
  $$("[data-flip]").forEach(c=>$$(":scope>[data-id]",c).forEach(e=>{
    const b=m.get(c.dataset.flip+":"+e.dataset.id),r=e.getBoundingClientRect();
    if(!b){if(had.has(c.dataset.flip)){e.classList.add("fresh");e.animate([{opacity:0,transform:"translateY(-12px) scale(.97)"},{opacity:1,transform:"none"}],{duration:340,easing:EASE})}}
    else{const dx=b.left-r.left,dy=b.top-r.top;if(dx||dy)e.animate([{transform:`translate(${dx}px,${dy}px)`},{transform:"none"}],{duration:340,easing:EASE})}}));
}
function collapse(el,cb){
  if(!el||reduced())return cb();const h=el.offsetHeight;el.style.overflow="hidden";el.style.minHeight="0";
  el.animate([{height:h+"px",opacity:1},{height:"0px",opacity:0,paddingTop:"0px",paddingBottom:"0px"}],{duration:240,easing:EASE,fill:"forwards"});setTimeout(cb,230);
}
function scramble(el){
  if(!el||reduced())return;const txt=el.textContent,ch="АБВГДЖЗИКЛМНПРСТФХЦЧШ01#/";let n=0;
  const id=setInterval(()=>{n++;if(!el.isConnected||n>=txt.length){el.textContent=txt;clearInterval(id);return}el.textContent=[...txt].map((c,i)=>i<n||c===" "?c:ch[Math.floor(Math.random()*ch.length)]).join("")},26);
}

const badgeText=c=>c.rule?`${SHORT[c.rule.type]||c.rule.type} ${c.rule.value}`:"default";
const routeText=c=>c.route==="proxy"?`VPN → ${(c.chains||[]).join(" ← ")||exitNode().name}`:c.route==="direct"?"Direct":"Blocked";
const routeClass=c=>c.origin==="default"?"r-default":"r-"+c.route;
const originWord={user:"your rule",plan:"plan rule",system:"system rule",default:"default"};
const isIP=h=>/^\d+\.\d+\.\d+\.\d+$/.test(h);
/* ── validation ───────────────────────────────────────────────────────── */
const DOM=/^(?=.{1,253}$)([a-z0-9а-я_]([a-z0-9а-я_-]{0,61}[a-z0-9а-я_])?\.)*[a-z0-9а-я]([a-z0-9а-я-]{0,61}[a-z0-9а-я])?$/;
function normalize(type,raw){
  let v=raw.trim(),note="";
  if(type==="DOMAIN"||type==="DOMAIN-SUFFIX"){if(/^[a-z]+:\/\//i.test(v)){try{v=new URL(v).hostname;note="Using the host from the link."}catch{}}v=v.toLowerCase().replace(/^\*?\./,"").replace(/\/.*$/,"")}
  if(type==="IP-CIDR"&&isIP(v))v+="/32";
  return{v,note};
}
function validate(type,raw,skipId){
  const{v,note}=normalize(type,raw);if(!v)return{v,err:"",warn:"",note:""};
  let err="";
  if((type==="DOMAIN"||type==="DOMAIN-SUFFIX")&&!DOM.test(v))err=`“${raw.trim()}” is not a domain.`;
  if(type==="DOMAIN-KEYWORD"&&/[\s,]/.test(v))err="A keyword cannot contain spaces or commas.";
  if(type==="IP-CIDR"){const m=v.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)\/(\d+)$/);if(!m||m.slice(1,5).some(o=>+o>255)||+m[5]>32)err="Use an address like 203.0.113.7 or a range like 203.0.113.0/24."}
  if(type==="PROCESS-NAME"&&/[\\/\s,]/.test(v))err="Use the file name only, for example steam.exe.";
  if(type==="DST-PORT"){const m=v.match(/^(\d+)(?:-(\d+))?$/);if(!m||+m[1]<1||+(m[2]||m[1])>65535||+(m[2]||m[1])<+m[1])err="A port is a number from 1 to 65535, or a range like 8000-8100."}
  if(type==="GEOSITE"&&!/^[a-z0-9!@-]+$/.test(v))err="A category name uses lowercase letters, digits and dashes.";
  if(err)return{v,err,warn:"",note};
  const others=S.rules.filter(r=>r.id!==skipId),n=r=>S.rules.indexOf(r)+1,dup=others.find(r=>r.type===type&&r.value===v);
  if(dup)return{v,err:`You already have this rule (rule ${n(dup)}).`,warn:"",note};
  const dom=type==="DOMAIN"||type==="DOMAIN-SUFFIX";
  const sh=others.find(r=>r.on&&dom&&((r.type==="DOMAIN-SUFFIX"&&v.endsWith("."+r.value))||(r.type==="DOMAIN-KEYWORD"&&v.includes(r.value))));
  return{v,err,warn:sh&&!skipId?`Rule ${n(sh)} already matches this. Put the new rule above it, or it will never run.`:"",note};
}
function parseRaw(line){
  const p=line.split(",").map(s=>s.trim());
  if(p.length<3)return{err:"Write a full rule: TYPE,value,target. For example DOMAIN-SUFFIX,example.com,DIRECT"};
  const type=p[0].toUpperCase();if(!SHORT[type])return{err:`“${p[0]}” is not a rule type this editor knows.`};
  const t=p[2].toUpperCase();return{type,value:p[1],target:t==="DIRECT"?"@direct":t.startsWith("REJECT")?"@block":"@proxy"};
}

/* ── toasts ───────────────────────────────────────────────────────────── */
function toast(text,actions=[]){
  const el=document.createElement("div");el.className="toast";el.innerHTML=`<span>${esc(text)}</span>`;
  const out=()=>{if(!el.isConnected)return;el.classList.add("out");setTimeout(()=>el.remove(),200)};
  actions.forEach(([label,fn])=>{const b=document.createElement("button");b.className="btn sm quiet";b.textContent=label;b.onclick=()=>{fn();out()};el.append(b)});
  $("#toasts").append(el);setTimeout(out,actions.length?8000:3600);
}

/* ── connection state ─────────────────────────────────────────────────── */
const blockReason=()=>null;
let connT=0;
function setConn(state){
  S.conn=state;$("#win").dataset.conn=state;
  const c=$("#ctl");if(c){c.dataset.state=state;c.setAttribute("aria-pressed",state==="connected");const a=$("#darc");if(a)a.style.strokeDashoffset=state==="connected"?"0":"";
    const t=$("#dtxt");if(t){t.textContent={idle:"STANDBY",connecting:"LINKING",connected:"ONLINE",blocked:"BLOCKED"}[state];$("#dsub").textContent={idle:"PRESS TO LINK",connecting:"HANDSHAKE",connected:"PRESS TO DROP",blocked:"SEE BELOW"}[state]}}
  renderStatus();renderBar();
}
let connectAttempt=0;
async function toggle(){
  if(S.busy){
    if(S.conn==="connecting"){const attempt=++connectAttempt;await invoke("disconnect");if(attempt===connectAttempt){S.busy=false;S.conns=[];connEls.clear();setConn("idle");if(S.screen==="home")renderSide()}}
    return;
  }
  if(S.conn!=="connected"&&S.signed_in&&sub()?.status==="active"&&S.engine_sub_id!==S.activeSub)await useSubscription(S.activeSub);
  if(S.conn!=="connected"&&!S.has_sub){go(S.signed_in?"subs":"account");return}
  const attempt=++connectAttempt;S.busy=true;const connected=S.conn==="connected";setConn(connected?"connected":"connecting");
  try{await invoke(connected?"disconnect":"connect");if(attempt!==connectAttempt)return;S.secs=0;S.conns=[];connEls.clear();setConn(connected?"idle":"connected");if(!connected)await loadNodes().catch(report);await watchTraffic()}
  catch(error){if(attempt!==connectAttempt)return;if(String(error)==="admin_required"){setConn("idle");await invoke("elevate")}else{setConn("idle");throw error}}
  finally{if(attempt===connectAttempt){S.busy=false;if(S.screen==="home")renderSide()}}
}
function renderStatus(){
  const el=$("#status");if(!el)return;const b=blockReason(),n=exitNode(),w=$("#stword");
  if(w)w.innerHTML=`<span class="swap">${{idle:"OFF",connecting:"···",connected:"ON",blocked:"NO"}[S.conn]}</span>`;
  if(S.conn==="blocked"&&b)el.innerHTML=`<div class="swap"><div class="s1" style="color:var(--warn)">${esc(b.t)}</div><div class="row" style="margin-top:10px;flex-wrap:wrap">${b.a.map(([l,a],i)=>`<button class="btn ${i?"":"primary"}" data-act="${a}">${l}</button>`).join("")}</div></div>`;
  else if(S.conn==="connected")el.innerHTML=`<div class="swap"><div class="s1">Connected</div><div class="timer" id="timer">${clock(S.secs)}</div><div class="s2">${S.node==="auto"?"Automatic · ":""}${esc(n.name)} ${n.sub}</div><div class="spd mono"><span>↓ <b id="dn">0</b></span><span>↑ <b id="up">0</b></span></div></div>`;
  else if(S.conn==="connecting")el.innerHTML=`<div class="swap"><div class="s1">Connecting…</div><div class="s2">Press again to cancel</div></div>`;
  else el.innerHTML=`<div class="swap"><div class="s1">Not connected</div><div class="s2">Tap to connect</div></div>`;
}
function renderBar(){
  const n=exitNode(),on=S.conn==="connected";
  $("#sbar").innerHTML=`<span><span class="led">${on?"●":"○"}</span> <b>${{idle:"standby",connecting:"linking",connected:"online",blocked:"blocked"}[S.conn]}</b></span><span>core <b>mihomo</b></span><span>mode <b>${S.mode==="tun"?"tun":"proxy"}</b></span><span>node <b>${on?n.sub:"—"}</b></span><span>↓ <b id="bdn">0</b></span><span>↑ <b id="bup">0</b></span><span style="margin-left:auto">up <b id="bt">${clock(on?S.secs:0)}</b></span>`;
}

/* ── living centre: dust that forms the logo, and a ring of lines ─────── */
I.gift='<rect x="3.5" y="8.5" width="13" height="8" rx="1.5"/><path d="M2.8 5.8h14.4v2.7H2.8zM10 5.8v10.7M10 5.8C8.6 2.9 5.6 3.3 6 5.8M10 5.8c1.4-2.9 4.4-2.5 4 0"/>';
let INK="#fff",ACC="#fff";
function readTheme(){const cs=getComputedStyle(root);INK=cs.getPropertyValue("--ink").trim();ACC=cs.getPropertyValue("--acc").trim()}
const N=900,PT=Array.from({length:N},()=>({a:Math.random()*6.2832,r:.74+Math.random()*.3+(Math.random()<.14?Math.random()*.3:0),z:Math.random(),ph:Math.random()*6.2832,s:.5+Math.random()*1.25,x:0,y:0,init:false}));
let MP=[];
function resample(){
  const c=document.createElement("canvas");c.width=c.height=256;const g=c.getContext("2d");g.scale(4,4);g.lineCap=g.lineJoin="round";g.strokeStyle=g.fillStyle="#fff";
  MARKS[S.logo].p.forEach(([d,w])=>{g.lineWidth=w;g.stroke(new Path2D(d))});MARKS[S.logo].c.forEach(([x,y,r])=>{g.beginPath();g.arc(x,y,r,0,7);g.fill()});
  const d=g.getImageData(0,0,256,256).data,pts=[];for(let y=0;y<256;y+=2)for(let x=0;x<256;x+=2)if(d[(y*256+x)*4+3]>128)pts.push([x/256-.5,y/256-.5]);
  MP=PT.map(()=>pts[Math.floor(Math.random()*pts.length)]);
}
function dust(g,W,dt){
  const c=W/2,R=W*.37,st=S.conn,ease=r=>1-Math.exp(-dt*r);
  PT.forEach((p,i)=>{
    let tx,ty,al,k;
    if(st==="connected"){const m=MP[i];tx=c+m[0]*W*.92+Math.sin(T*1.3+p.ph)*.7;ty=c+m[1]*W*.92+Math.cos(T*1.1+p.ph)*.7;al=.5+.5*Math.sin(T*1.8+p.ph)**2;k=ease(4.5)}
    else if(st==="connecting"){const a=p.a+T*2.6;tx=c+Math.cos(a)*R*(.84+.1*Math.sin(p.ph));ty=c+Math.sin(a)*R*(.84+.1*Math.sin(p.ph));al=.85;k=ease(6)}
    else{const a=p.a+T*.14*(1.5-p.r*.5),dx=Math.cos(a)*R*p.r,dy=Math.sin(a)*R*p.r*.34+(p.z-.5)*9;tx=c+dx*.951+dy*.309;ty=c-dx*.309+dy*.951;al=(.18+.6*(.5+.5*Math.sin(a)))*(st==="blocked"?.55:1);k=ease(3)}
    if(!p.init||reduced()){p.x=tx;p.y=ty;p.init=true}else{p.x+=(tx-p.x)*k;p.y+=(ty-p.y)*k}
    g.globalAlpha=al;g.fillStyle=st==="connected"&&p.s>1.25?ACC:INK;g.beginPath();g.arc(p.x,p.y,st==="connected"?p.s*.8:p.s,0,6.2832);g.fill();
  });
}
let amp=0;
function ticks(g,W){
  const c=W/2,R=W*.36,n=108,st=S.conn,dn=S.conns.reduce((a,x)=>a+x.dn,0);
  amp+=((st==="connected"?Math.min(1,.3+dn/4e6):0)-amp)*.05;g.lineCap="round";
  for(let i=0;i<n;i++){
    const a=i/n*6.2832-1.5708;let len=5,al=.3;
    if(st==="connecting"){const d=Math.abs((((a-T*4.2)%6.2832)+9.4248)%6.2832-3.1416),h=Math.max(0,1-d/1.3);len=5+20*h;al=.3+.7*h}
    else if(st==="connected"){const w=.5+.5*Math.sin(i*.41+T*1.7)*Math.sin(i*.13-T*.9);len=6+amp*24*Math.max(.12,w);al=.92}
    else if(st==="blocked")al=.18;
    g.globalAlpha=al;g.strokeStyle=st==="connected"?ACC:INK;g.lineWidth=1.5;g.beginPath();g.moveTo(c+Math.cos(a)*R,c+Math.sin(a)*R);g.lineTo(c+Math.cos(a)*(R+len),c+Math.sin(a)*(R+len));g.stroke();
  }
}
let T=0,lastT=performance.now(),frameHandle=0;
function startAnimation(){if(S.signed_in&&!frameHandle&&!document.hidden&&S.screen==="home"){lastT=performance.now();frameHandle=requestAnimationFrame(frame)}}
function frame(now){
  frameHandle=0;if(!S.signed_in||document.hidden||S.screen!=="home")return;frameHandle=requestAnimationFrame(frame);const cv=$("#cv");if(!cv)return;
  const dt=Math.min(.05,(now-lastT)/1000);lastT=now;if(!reduced())T+=dt;
  const W=cv.clientWidth,dpr=Math.min(2,devicePixelRatio||1);if(!W)return;if(cv.width!==Math.round(W*dpr))cv.width=cv.height=Math.round(W*dpr);
  const g=cv.getContext("2d");g.setTransform(dpr,0,0,dpr,0,0);g.clearRect(0,0,W,W);
  dust(g,W,dt);g.globalAlpha=1;
}
function buildShell(){
  const v=VAR[S.v];$("#word").innerHTML=v.word;$("#logo").innerHTML=markSVG(S.logo);$("#themebtn").hidden=v.themes.length<2;
  $("#rail").innerHTML='<span class="nav-ind" id="ind"></span>';markNav();renderBar();
}
function markNav(){
  const ab=$("#accbtn");if(ab)ab.setAttribute("aria-pressed",S.screen!=="home");
  const cur=isMobile()&&["subs","devices","settings"].includes(S.screen)?"account":S.screen;
  $$("#rail .nav").forEach(b=>b.dataset.v===cur?b.setAttribute("aria-current","page"):b.removeAttribute("aria-current"));
  const a=$("#rail .nav[aria-current]"),ind=$("#ind");
  if(a&&a.offsetParent)ind.style.cssText=`left:${a.offsetLeft}px;top:${a.offsetTop}px;width:${a.offsetWidth}px;height:${a.offsetHeight}px;opacity:1`;else ind.style.opacity=0;
}
const offline=()=>S.me?.maintenance?.enabled?`<div class="banner">${ic("warn","s")}<span>${esc(S.me.maintenance.message||"The account service is under maintenance.")}</span></div>`:"";
const head=(t,extra="")=>`<div class="head"><button class="btn back" data-act="go" data-v="${S.screen==="account"?"home":"account"}" aria-label="Back">${ic("back")}</button><h1 class="title">${t}</h1><span class="sp"></span>${extra}</div>`;
const MODES=[["tun","Whole device"],["proxy","Browser only"]];
function ruleTypePicker(){
  return `<details class="rule-types" id="ctype"><summary class="select" aria-label="Rule type"><span id="ctype-label" class="grow">${esc(TYPES.find(t=>t[0]===S.comp.type)[1])}</span>${ic("chev","s")}</summary><div class="rule-type-list" role="group" aria-label="Choose rule type">${TYPES.map(t=>`<button class="btn" type="button" data-act="rule-type" data-v="${t[0]}" aria-pressed="${S.comp.type===t[0]}">${esc(t[1])}${ic("check","s")}</button>`).join("")}</div></details>`;
}

const screens={
  home(){const k=VAR[S.v].ctl;return`${offline()}<div class="home"><section class="hero" aria-label="Connection">
      <button class="ctl ${k}" id="ctl" data-act="toggle" data-state="${S.conn}" aria-pressed="${S.conn==="connected"}" aria-label="Connect or disconnect" aria-describedby="status">${CTL[k]()}</button>
      <div class="status" id="status" aria-live="polite"></div></section>
    <aside class="side stag" id="side"></aside></div>`},

  conns(){
    if(S.conn!=="connected")return head("Connections")+`<div class="card empty"><p>Nothing is flowing. Connect to see where your traffic goes, and which rule sent it there.</p><button class="btn primary" data-act="connect-here">Connect</button></div>`;
    return head("Connections",`<span class="cap mono" id="ctot"></span>`)+`<div class="tools">${seg("filter",[["all","All"],["proxy","Via VPN"],["direct","Direct"],["block","Blocked"]],S.filter,"filter")}
      <input class="input grow" id="q" placeholder="Search host or app" value="${esc(S.q)}" aria-label="Search connections">
      <button class="btn" data-act="pause" aria-pressed="${S.paused}">${ic(S.paused?"play":"pause","s")}${S.paused?"Resume":"Pause"}</button></div>
      <div class="card clip"><button class="btn sm primary newpill" id="newpill" data-act="flush" hidden></button><div class="list" id="clist" role="list"></div></div>
      <p class="cap dim" style="margin:12px 6px 0">The mark on a badge shows where the rule comes from: <span class="badge o-user r-direct" style="vertical-align:middle"><i class="dot"></i><span>yours</span></span> <span class="badge o-plan r-proxy" style="vertical-align:middle"><i class="dot"></i><span>your plan</span></span> <span class="badge o-system r-direct" style="vertical-align:middle"><i class="dot"></i><span>system</span></span> <span class="badge o-default r-default" style="vertical-align:middle"><span>default</span></span></p>`},

  rules(){
    const c=S.comp,on=S.rules.filter(r=>r.on).length;
    const comp=c.raw?`<div class="composer" style="grid-template-columns:minmax(0,1fr) auto auto"><input class="input mono" id="rawv" placeholder="DOMAIN-SUFFIX,example.com,DIRECT" value="${esc(c.value)}" aria-label="Raw rule"><button class="btn primary" data-act="rule-add" id="addbtn">Add</button><button class="btn quiet" data-act="comp-mode">Simple</button></div>`
      :`<div class="composer">${ruleTypePicker()}
        <input class="input mono" id="cval" placeholder="${TYPES.find(t=>t[0]===c.type)[3]}" value="${esc(c.value)}" aria-label="Value" aria-describedby="cmsg">
        ${seg("ctarget",Object.entries(TARGET).map(([k,[l]])=>[k,l]),c.target,"comp-target")}
        <div class="row" style="gap:4px"><button class="btn primary" data-act="rule-add" id="addbtn">${ic("plus","s")}Add</button><button class="btn quiet" data-act="comp-mode">Raw</button></div></div>`;
    const rows=S.rules.length?S.rules.map((r,i)=>S.edit===r.id?`<div class="rule" data-id="${r.id}" style="grid-template-columns:20px 22px 86px minmax(0,1fr) auto auto"><span></span><span class="mono dim idx">${i+1}</span><span class="tchip">${SHORT[r.type]}</span><input class="input mono" id="editv" value="${esc(r.value)}" aria-label="Edit value"><button class="btn sm primary" data-act="edit-save" data-id="${r.id}">Save</button><button class="btn sm quiet" data-act="edit-cancel">Cancel</button></div>`
      :`<div class="rule ${r.on?"":"off"}" draggable="true" data-id="${r.id}" role="listitem">
        <span class="grip" title="Drag to reorder">${ic("grip","s")}</span><span class="mono dim idx">${i+1}</span><span class="tchip">${SHORT[r.type]}</span>
        <button class="host mono" data-act="edit" data-id="${r.id}" title="Edit value">${esc(r.value)}</button>
        <button class="pill" style="--c:var(--${TARGET[r.target][1]})" data-act="rule-target" data-id="${r.id}" title="Change target">${TARGET[r.target][0]}</button>
        <span class="mono dim hits" title="Live match counts are unavailable">${r.hits||"·"}</span>
        <button class="sw" role="switch" data-k="rule${r.id}" aria-checked="${r.on}" data-act="rule-toggle" data-id="${r.id}" aria-label="Rule ${i+1} enabled"></button>
        <span class="row x" style="gap:0"><button class="btn sm quiet" data-act="rule-move" data-id="${r.id}" data-d="-1" aria-label="Move up" ${i?"":"disabled"}>↑</button><button class="btn sm quiet" data-act="rule-move" data-id="${r.id}" data-d="1" aria-label="Move down" ${i<S.rules.length-1?"":"disabled"}>↓</button><button class="btn sm quiet" data-act="rule-del" data-id="${r.id}" aria-label="Delete rule ${i+1}">${ic("trash","s")}</button></span></div>`).join("")
      :`<div class="empty"><p>No rules of your own yet. Add one above, or from any live connection.</p></div>`;
    const ro=list=>list.map((r,i)=>`<div class="rule"><span class="mono dim">${i+1}</span><span class="tchip">${SHORT[r.type]}</span><span class="host mono">${esc(r.value)}</span><span class="pill" style="--c:var(--${TARGET[r.target][1]})">${TARGET[r.target][0]}</span></div>`).join("");
    return head("Rules",`<span class="cap" id="applied">${ic("check","s")} ${S.applied} · ${on} active</span>`)+`<div class="col">
      <div class="card pad"><div class="over" style="margin-bottom:10px">New rule</div>${comp}<div class="msg" id="cmsg" role="status"></div></div>
      <div><div class="row" style="margin:4px 6px 8px"><span class="over">Your rules</span><span class="cap dim">First match wins. Yours run before your plan's.</span></div>
      <div class="card clip"><div class="list stag" id="rlist" role="list" data-flip="rules">${rows}</div></div></div>
      <div class="cap dim">Subscription and system routes are reported by the running core in Traffic.</div></div>`},

  subs(){
    return head("Plans",`<button class="btn" data-act="subscription-refresh" ${!S.has_sub||S.refreshing?"disabled":""}>${S.refreshing?"Updating…":"Update subscription"}</button><button class="btn" data-act="pay-buy">${ic("plus","s")}New plan</button>`)+offline()+`${S.pay?.step==="wait"&&S.pay.kind!=="topup"?'<div class="banner"><span class="grow">A payment is awaiting confirmation.</span><button class="btn sm" data-act="resume-payment">View payment</button></div>':""}<div class="cards stag" data-flip="subs">${S.subs.map(s=>{
      const act=s.status==="active",soon=act&&s.days<=3,inUse=s.id===S.activeSub,left=Math.max(0,s.gb-s.used),devs=S.deviceLoaded[s.id]?S.devices[s.id].length:null;
      const pill=!act?`<span class="pill bad">Expired</span>`:soon?`<span class="pill warn">Ends in ${s.days} days</span>`:s.plan==="Trial"?`<span class="pill">Trial</span>`:`<span class="pill ok">Active</span>`;
      const full=s.devices>0&&devs!==null&&devs>=s.devices&&!S.devices[s.id].some(d=>d.me);
      return`<article class="card pad plan col ${act?"":"expired"} ${inUse?"inuse":""}" data-id="${s.id}" aria-label="${esc(s.plan)}">
        <div class="row"><h2 class="h grow">${esc(s.plan)}</h2>${inUse?`<span class="pill acc ${S.just==="use"+s.id?"pop":""}">${ic("check","s")}In use</span>`:""}${pill}</div>
        <div class="kv"><div><div class="num">${num("d"+s.id,s.days)}<small>days left</small></div><div class="meter" style="--c:var(--${soon?"warn":"ink2"});margin-top:8px"><i style="--v:${s.term?Math.min(100,s.days/s.term*100):0}%"></i></div></div>
          <div><div class="num">${s.gb?num("g"+s.id,Math.round(left)):"∞"}<small>${s.gb?"GB left":"traffic"}</small></div>${s.gb?`<div class="meter" style="margin-top:8px"><i style="--v:${left/s.gb*100}%"></i></div>`:""}</div></div>
        <div class="cap">${act?"Until "+s.expires:"Ended "+s.expires} · ${devs===null?"—":devs} of ${s.devices||"unlimited"} devices</div>
        ${S.pendingSwitch===s.id?(full?`<div class="swap"><div class="banner" style="margin:0 0 10px">All ${s.devices} device slots on this plan are taken.</div><div class="row"><button class="btn primary" data-act="go-devices">Manage devices</button><button class="btn quiet" data-act="switch-cancel">Cancel</button></div></div>`
          :`<div class="swap"><div class="cap" style="margin-bottom:10px">Switch now? ${S.conn==="connected"?"You will reconnect in a moment.":""}</div><div class="row"><button class="btn primary" data-act="switch-ok" data-id="${s.id}">Switch</button><button class="btn quiet" data-act="switch-cancel">Cancel</button></div></div>`)
        :`<div class="row plan-actions">${s.tariff||!act?`<button class="btn ${inUse||!act?"primary":""}" data-act="pay-renew" data-id="${s.id}">${s.tariff?"Renew":"Choose a plan"}</button>`:""}${act&&s.gb>0?`<button class="btn" data-act="pay-traffic" data-id="${s.id}">Add traffic</button>`:""}${act&&!inUse?`<button class="btn" data-act="switch" data-id="${s.id}">Use this plan</button>`:""}</div>`}
      </article>`}).join("")}${S.subs.length?"":'<div class="card empty"><p>No plans yet. Choose a plan to get connected.</p><button class="btn primary" data-act="pay-buy">Choose a plan</button></div>'}</div>`},

  devices(){
    const s=sub();if(!s)return head("Devices")+'<div class="card empty"><p>Choose a plan to manage its devices.</p><button class="btn primary" data-act="go" data-v="subs">Plans</button></div>';if(!S.deviceLoaded[s.id])return head("Devices")+'<div class="card empty">Loading devices…</div>';const list=S.devices[s.id]||[],max=s.devices;
    return head("Devices")+offline()+`<div class="card pad" style="margin-bottom:14px"><div class="row"><div class="grow"><div class="h">${num("devn",list.length)} of ${max?num("devm",max):"unlimited"} devices</div><div class="cap">${esc(s.plan)} plan. A device is added the first time it connects.</div></div>${max>0?`<button class="btn" data-act="pay-slot">${ic("plus","s")}Add a slot</button>`:""}</div>
      <div class="slots" style="margin-top:14px" aria-hidden="true">${Array.from({length:Math.min(max||list.length,100)},(_,i)=>`<i class="${i<list.length?(list[i].me?"me":"on"):""}"></i>`).join("")}</div></div>
      <div class="card clip"><div class="list stag" data-flip="devs">${list.length?list.map(d=>`<div class="dev" data-id="${esc(d.hwid)}">
        <span class="cc">${ic(d.kind,"s")}</span>
        <div class="grow">${S.edit==="d"+d.hwid?`<input class="input" id="devname" value="${esc(d.name)}" data-id="${esc(d.hwid)}" aria-label="Device name" maxlength="64">`:`<button class="nameBtn" data-act="dev-edit" data-id="${esc(d.hwid)}" title="Rename">${esc(d.name)}</button>${d.me?` <span class="pill acc">This device</span>`:""}`}<div class="cap dim">${esc(d.meta)}</div></div>
        <span class="cap dim added">${d.added?"Added "+esc(d.added):""}</span>
        ${S.edit==="x"+d.hwid?`<span class="row swap" style="gap:6px"><span class="cap">${d.me?"You will be disconnected here.":"Remove it?"}</span><button class="btn sm danger" data-act="dev-del" data-id="${esc(d.hwid)}">Remove</button><button class="btn sm quiet" data-act="edit-cancel">Keep</button></span>`:`<button class="btn sm quiet" data-act="dev-ask" data-id="${esc(d.hwid)}">Revoke</button>`}
      </div>`).join(""):`<div class="empty"><p>No devices yet. This computer is added the first time you connect.</p></div>`}</div></div>`},

  settings(){
    const p=S.profile||{};
    return head("Settings")+`<div class="groups stag">
      ${group("Connection",`<div class="set mode-setting"><div class="grow"><div class="set-t">Mode</div><div class="cap dim">Whole device uses a virtual adapter. Browser only sets the system proxy.</div></div>${seg("mode",MODES,S.mode,"mode")}</div>`+`<div class="set"><div class="grow"><div class="set-t">Device identification</div><div class="cap dim">Hardware identification is required and cannot be disabled.</div></div><span class="pill ${S.hwidAvailable===false?"bad":"ok"}">${S.hwidAvailable===false?"Unavailable":"Required"}</span></div>`+navrow("rules","rules","Routing rules",S.rules.filter(r=>r.on).length+" of your own")+navrow("conns","conns","Traffic",S.conn==="connected"?S.conns.length+" open connections":"Where your traffic goes"))}
      <section class="grp"><h2 class="over">Subscription</h2>${subscriptionInfo()}</section>
      ${group("Application",`<div class="set"><div class="grow"><div class="set-t">Keep running in the tray</div><div class="cap dim">${S.closeToTray?"Closing the window hides it. Quit from the tray menu.":"Closing the window disconnects and quits."}</div></div><button class="sw" role="switch" data-k="closetray" aria-checked="${S.closeToTray}" data-act="close-tray" aria-label="Keep running in the tray"></button></div>`)}
      ${group("About",inforow("Version",S.version||"")+inforow("Core","mihomo · Windows")+(p.update_hours?inforow("Profile refresh",p.update_hours+(p.update_hours===1?" hour":" hours")):""))}
      ${group("Legal",legalRows())}</div>`
  },
  account(){
    if(!S.signed_in)return signInScreen();
    if(!S.me)return head("Account")+'<div class="card empty"><p>Your account service is temporarily unavailable.</p><button class="btn primary" data-act="account-refresh">Retry</button></div>';
    const s=sub(),support=S.profile?.support_url,name=accountName(),active=s&&s.status==="active",soon=active&&s.days<=3,left=s?Math.max(0,s.gb-s.used):0;
    const pill=!s?"":!active?'<span class="pill bad">Expired</span>':soon?`<span class="pill warn">Ends in ${s.days} days</span>`:s.plan==="Trial"?'<span class="pill">Trial</span>':'<span class="pill ok">Active</span>';
    const action=!s?'<button class="btn primary block" data-act="pay-buy">Choose a plan</button>':s.tariff||!active?`<button class="btn primary block" data-act="pay-renew" data-id="${s.id}">${s.tariff?"Renew":"Choose a plan"}</button>`:'<button class="btn block" data-act="go" data-v="subs">All plans</button>';
    return head("Account")+`<div class="groups stag">
      <section class="grp profile"><div class="card pcard col">
        <div class="row"><span class="avatar" aria-hidden="true">${esc((name.trim()[0]||"a").toUpperCase())}</span><div class="grow"><div class="pname">${esc(name)}</div><div class="cap dim">${S.me?.telegram_linked?"Telegram linked":"Your account"}</div></div><button class="btn sm quiet danger" data-act="logout">${ic("logout","s")}Sign out</button></div>
        <div class="profile-plan ${s?"":"no-plan"}">${s?`<div class="profile-plan-name"><h2 class="h">${esc(s.plan)}</h2>${pill}</div>
        <div class="kv"><div><div class="over">Time left</div><div class="num">${num("pd",s.days)}<small>days</small></div><div class="cap dim">${active?"until":"ended"} ${esc(s.expires)}</div></div>
          <div><div class="over">Traffic left</div><div class="num">${s.gb?num("pg",Math.round(left)):"∞"}<small>${s.gb?"GB":"unlimited"}</small></div></div></div>`:'<div class="cap">No plan yet. Choose one to get connected.</div>'}
        <div class="profile-action">${action}</div></div></div></section>
      ${group("Plan",navrow("subs","plans","Plans",s?s.plan+" · "+s.days+" days left":"Choose your plan")+navrow("devices","devices","Devices",s?(S.deviceLoaded[s.id]?S.devices[s.id].length+" of "+(s.devices||"unlimited"):"Manage connected devices"):"Your connected devices"))}
      ${group("Payments",navrow("balance","wallet","Balance",rub(S.balance)))}
      ${group("Connection",navrow("conns","conns","Traffic",S.conn==="connected"?S.conns.length+" open connections":"Where your traffic goes")+navrow("rules","rules","Routing rules",S.rules.filter(r=>r.on).length+" of your own"))}
      ${group("Bonuses",navrow("referral","gift","Referral program","Invite friends, earn balance")+navrow("promo","tag","Promo code","Add a code to your account","promo"))}
      ${group("Application",navrow("settings","settings","Settings","Connection, documents, about")+(support?`<button class="set navrow" data-act="profile-link" data-v="support_url">${ic("help")}<span class="grow set-t">Support</span>${ic("ext","s")}</button>`:""))}</div>`
  },
  balance(){
    return head("Balance")+offline()+`<section class="card pad balance-card"><div><div class="over">Available balance</div><div class="num">${num("bal",S.balance,"rub")}</div><p class="cap dim">Use your balance for plans, traffic and device slots.</p></div><button class="btn primary" data-act="pay-topup">${ic("plus","s")}Top up</button></section>`+(S.pay?.kind==="topup"&&S.pay.step==="wait"?'<div class="banner" style="margin-top:16px"><span class="grow">A top-up is awaiting confirmation.</span><button class="btn" data-act="resume-payment">View payment</button></div>':"")+historyCard()
  },
  referral(){
    const r=S.referral;
    return head("Referrals")+(r?`<div class="col stag"><div class="card pad col"><span class="over">Your link</span><div class="row"><input class="input mono grow" readonly value="${esc(r.ref_link_web||r.ref_link)}" aria-label="Referral link"><button class="btn primary" data-act="copy-ref">Copy</button></div><div class="cap dim">${esc(r.join_bonus_formatted?"A friend receives "+r.join_bonus_formatted+". ":"")}${esc(r.referral_bonus_formatted?"Your reward: "+r.referral_bonus_formatted:"")}</div></div><div class="card pad"><div class="kv"><div><div class="num">${num("rf1",r.count)}<small>invited</small></div></div><div><div class="num">${num("rf2",r.earned_kopecks,"rub")}</div><div class="cap dim">earned so far</div></div></div></div></div>`:'<div class="card empty">Loading referral details…</div>')
  }

};

function renderSide(){
  const el=$("#side");if(!el)return;const s=sub(),n=nodeOf(S.node),left=s?Math.max(0,s.gb-s.used):0,soon=s&&s.days<=3,exp=s&&s.status!=="active";
  el.innerHTML=`${S.profile?.announcement?`<section class="card pad col announcement"><h2 class="over">Announcement</h2><p class="cap">${esc(S.profile.announcement)}</p></section>`:""}<button class="card pad row" data-act="nodes" style="text-align:left;width:100%" aria-label="Choose a node"><span class="cc">${nodeFlag(n.id)}</span><span class="grow"><span style="font-weight:600;display:block">${esc(n.name)}</span><span class="cap dim">${esc(n.sub)}</span></span>${ic("chev","s")}</button>
    <button class="card pad row" data-act="go" data-v="subs" style="text-align:left;width:100%" aria-label="Your plan"><span class="grow"><span style="font-weight:600;display:block">${s?esc(s.plan):S.signed_in?"Choose a plan":"Sign in to your account"}</span><span class="cap dim">${s?(exp?"Renew to connect":`${num("hd",s.days)} days · ${s.gb?num("hg",Math.round(left))+" GB left":"unlimited"}`):S.has_sub?"A connection profile is saved on this computer":"Your plans and devices in one place"}</span></span>${s?`<span class="pill ${exp?"bad":soon?"warn":"ok"}">${exp?"Expired":soon?"Ends soon":"Active"}</span>`:""}${ic("chev","s")}</button>
    <div class="card pad col only-d"><span class="over">Mode</span>${seg("mode",MODES,S.mode,"mode","wide")}</div>`;
  [...el.children].forEach((c,i)=>c.style.setProperty("--k",i+1));settle(el);
}
function render(){
  root.dataset.auth=S.signed_in?"signed-in":"signed-out";$("#logo").disabled=!S.signed_in;
  const screen=S.signed_in?S.screen:"account",m=$("#main");m.innerHTML=screens[screen]();startAnimation();
  [...m.children].forEach((c,i)=>c.style.setProperty("--k",i));
  $$(".stag",m).forEach(s=>[...s.children].forEach((c,i)=>c.style.setProperty("--k",Math.min(i,9)+1)));
  if(S.signed_in&&screen==="home"){setConn(S.conn);renderSide()}
  if(S.signed_in&&screen==="conns"&&S.conn==="connected"){connEls.clear();syncConns(true)}
  if(S.signed_in&&screen==="rules")showMsg();
  if(S.deviceBusy)for(const control of $$('[data-act^="dev-"],[data-act="switch"],[data-act="switch-ok"]',m))control.disabled=true;
  settle(m);S.just=null;
  const f=$("#editv")||$("#devname");if(f){f.focus();f.select()}
}
/* same-screen update: no screen animation, rows glide instead */
function rr(){const m=$("#main");m.classList.remove("enter","leave");const s=snap();render();play(s)}
let navT=0,enT=0,pendingPaint=null;const entered=()=>{clearTimeout(enT);enT=setTimeout(()=>{$("#main").classList.remove("enter");const screen=pendingPaint;pendingPaint=null;if(screen===S.screen)paintScreen(screen)},1100)};
function go(v){
  if(!S.signed_in)v="account";
  if(v===S.screen)return;const m=$("#main");m.dataset.dir=ORDER.indexOf(v)>ORDER.indexOf(S.screen)?"f":"b";
  S.screen=v;void refreshScreen(v).catch(report);void watchTraffic();S.edit=null;S.scopeFor=null;S.pendingSwitch=null;markNav();clearTimeout(navT);
  const swap=()=>{mem.n={};m.classList.remove("leave","enter");render();m.scrollTop=0;void m.offsetWidth;m.classList.add("enter");entered()};
  if(reduced())return swap();m.classList.add("leave");navT=setTimeout(swap,S.v===4?90:130);
}

/* ── live connections: keyed rows, never reorder under the pointer ────── */
const connEls=new Map();
function rowHTML(c){
  const has=t=>c.origin==="user"&&c.rule.target===t;
  return`<div class="host mono" title="${esc(c.proc)}">${esc(c.host)}<i>:${c.port}</i><div class="proc">${esc(c.proc)}</div></div>
    <div class="route ${routeClass(c)}">${esc(routeText(c))}</div>
    <div class="bwrap"><span class="badge o-${c.origin} ${routeClass(c)}" title="Matched ${originWord[c.origin]}"><i class="dot"></i><span>${esc(badgeText(c))}</span></span></div>
    <div class="rate"><span data-k="dn">↓ ${rate(c.dn)}</span><br><span data-k="up">↑ ${rate(c.up)}</span></div>
    <div class="acts" role="group" aria-label="Quick rule for ${esc(c.host)}">${[["@direct","Direct","Always direct"],["@proxy","Via VPN","Always via VPN"],["@block","Block","Always block"]].map(([t,l,full])=>`<button class="btn sm ${has(t)?"":"quiet"}" data-act="quick" data-id="${c.id}" data-v="${t}" title="${full}" aria-label="${full}">${has(t)?ic("check","s"):""}${l}</button>`).join("")}</div>${S.scopeFor&&S.scopeFor.id===c.id?scopeHTML(c):""}`;
}
function scopeOptions(c){
  if(isIP(c.host))return[["IP-CIDR",c.host+"/32","this address"],["PROCESS-NAME",c.proc,"everything from "+c.proc]];
  const base=c.host.split(".").slice(-2).join("."),o=[["DOMAIN-SUFFIX",base,base+" and subdomains"]];
  if(c.host!==base)o.push(["DOMAIN",c.host,"only "+c.host]);o.push(["PROCESS-NAME",c.proc,"everything from "+c.proc]);return o;
}
function scopeHTML(c){const sc=S.scopeFor;return`<div class="scope"><span class="cap">${TARGET[sc.target][0]}:</span>${scopeOptions(c).map((o,i)=>`<button class="chip" data-act="scope" data-i="${i}" aria-pressed="${sc.i===i}">${esc(o[2])}</button>`).join("")}<span class="grow"></span><button class="btn sm primary" data-act="scope-add" id="scopeadd">Add rule</button><button class="btn sm quiet" data-act="scope-cancel">Cancel</button></div>`}
const visible=c=>(S.filter==="all"||c.route===S.filter)&&(!S.q||(c.host+c.proc).toLowerCase().includes(S.q.toLowerCase()));
function processName(proc){return String(proc||'Unknown process').split(/[\\/]/).pop()||'Unknown process'}
function groupConnections(connections){const groups=new Map();for(const c of connections){const name=processName(c.proc),key=name.toLowerCase();if(!groups.has(key))groups.set(key,{key,name,connections:[],dn:0,up:0});const group=groups.get(key);group.connections.push(c);group.dn+=c.dn||0;group.up+=c.up||0}return [...groups.values()]}
const processEls=new Map();let processList=null;
function syncConns(force){
 const list=$('#clist');if(!list)return;
 if(processList!==list){processEls.clear();connEls.clear();processList=list;}
 const busy=!force&&(S.paused||list.matches(':hover')||list.contains(document.activeElement)||S.scopeFor);let pending=0;
 const groups=groupConnections(S.conns),active=new Set(S.conns.map(c=>c.id));
 for(const[id,el]of connEls)if(!active.has(id)){el.remove();connEls.delete(id)}
 for(const group of groups){
  let block=processEls.get(group.key);
  if(!block){if(busy){pending+=group.connections.length;continue}const details=document.createElement('details');details.className='process-group';details.open=true;const summary=document.createElement('summary');summary.className='process-summary';const rows=document.createElement('div');rows.className='process-connections';rows.setAttribute('role','list');details.append(summary,rows);list.append(details);block={details,summary,rows};processEls.set(group.key,block)}
  const shown=group.connections.filter(visible);block.details.hidden=!shown.length;
  block.summary.innerHTML=`<span class="grow">${esc(group.name)}</span><span class="cap dim">${shown.length} connections</span><span class="mono dim">↓ ${rate(group.dn)} · ↑ ${rate(group.up)}</span>`;
  for(const c of group.connections){let el=connEls.get(c.id);if(!el){if(busy){pending++;continue}el=document.createElement('div');el.className='conn'+(force?'':' new');el.setAttribute('role','listitem');el.dataset.id=c.id;el.innerHTML=rowHTML(c);block.rows.append(el);connEls.set(c.id,el)}else if(c.dirty){el.innerHTML=rowHTML(c);const badge=$('.badge',el);if(badge)badge.classList.add('flash')}else{const dn=$('[data-k="dn"]',el),up=$('[data-k="up"]',el);if(dn)dn.textContent='↓ '+rate(c.dn);if(up)up.textContent='↑ '+rate(c.up)}if(el.parentElement!==block.rows)block.rows.append(el);c.dirty=false;el.hidden=!visible(c)}
 }
 const keys=new Set(groups.map(g=>g.key));for(const[key,block]of processEls)if(!keys.has(key)){block.details.remove();processEls.delete(key)}
 const np=$('#newpill');np.hidden=!pending;np.textContent=pending+' new';
 $('#ctot').textContent=`${S.conns.length} open · ↓ ${rate(S.conns.reduce((a,c)=>a+c.dn,0))} · ↑ ${rate(S.conns.reduce((a,c)=>a+c.up,0))}`;
}

/* ── rules: add, apply, undo ──────────────────────────────────────────── */
let savedRules=[],rulesQueue=Promise.resolve();
function applyRules(){
  const rules=S.rules.map(({id,type,value,target,on})=>({id,type,value,target,on}));S.applied="Applying…";
  rulesQueue=rulesQueue.then(()=>invoke("set_rules",{rules})).then(()=>{savedRules=rules;S.applied="Applied";const el=$("#applied");if(el)el.innerHTML=`<span class="swap">${ic("check","s")} Applied · ${rules.filter(r=>r.on).length} active</span>`}).catch(error=>{S.rules=savedRules.map(r=>({...r}));S.applied="Not saved";if(S.screen==="rules")rr();report(error)});return rulesQueue;
}
const affected=()=>[];
function reapply(){};

function addRule(type,value,target,atTop=true){
  const r={id:++uid,type,value,target,on:true,hits:0};atTop?S.rules.unshift(r):S.rules.push(r);applyRules();
  const n=affected().length;
  toast(`Rule added: ${value} → ${TARGET[target][0]}`,[["Undo",()=>{S.rules=S.rules.filter(x=>x!==r);applyRules();reapply();if(S.screen==="rules")rr()}],...(n?[[`Apply to ${n} open`,reapply]]:[])]);
  return r;
}
function showMsg(){
  const el=$("#cmsg");if(!el)return;const c=S.comp;
  if(c.raw){el.className="msg ok";el.textContent=c.value?"":"Paste or type one rule line. It is checked before it is applied.";return}
  const r=validate(c.type,c.value),cls="msg "+(r.err?"err":r.warn?"warn":"ok"),txt=r.err||r.warn||r.note||"";
  if(el.textContent!==txt||el.className!==cls){el.className=cls;el.textContent=txt}
  const b=$("#addbtn");if(b)b.disabled=!!r.err||!r.v;
}

/* ── sheets: payment and node picker ──────────────────────────────────── */
const STEP={plan:0,method:1,wait:2,done:3,fail:3};
function promoBody(){return{title:"Promo code",body:`<form id="promo" class="col"><label for="promo-code" class="cap">Enter your code</label><input class="input mono" id="promo-code" autocomplete="off" spellcheck="false" maxlength="64" required><p class="cap dim">A code can add balance, days or a discount to your account.</p><button class="btn primary block" type="submit">Apply code</button></form>`,foot:""}}
/* Real account and checkout adapters below keep the prototype's sheet animation plumbing. */

const msHTML=n=>n.id==="auto"?"":`<span class="mono ms" data-ms="${esc(n.id)}" style="color:var(--${n.status==="timeout"?"bad":n.ms==null?"ink3":n.ms<120?"ok":"warn"})">${n.ms==null?(n.status==="timeout"?"timeout":"Not tested"):n.ms+" ms"}</span>`;
function nodesBody(){
  return{title:"Choose a node",body:`<div class="col" style="gap:8px">${NODES.map(n=>`<button class="opt ${S.just==="n"+n.id?"just":""}" data-act="node" data-v="${esc(n.id)}" aria-pressed="${S.node===n.id}">${tickEl}<span class="cc">${nodeFlag(n.id)}</span><span class="grow"><span style="font-weight:600;display:block">${esc(n.name)}</span><span class="cap dim">${esc(n.sub)}</span></span>${msHTML(n)}</button>`).join("")}</div>`,
    foot:`<div class="row"><button class="btn grow" data-act="subscription-refresh" ${S.refreshing||S.testing?"disabled":""}>${S.refreshing?"Updating…":"Update subscription"}</button><button class="btn grow ${S.testing?"loading":""}" data-act="test" ${S.testing||S.refreshing?"disabled":""}>${S.testing?"Testing…":"Test latency"}</button></div>${S.conn!=="connected"?'<p class="cap dim">Your choice will be used when you connect.</p>':""}`};
}
let lastStep=-1;
function renderSheet(){
  const el=$("#sheet");if(!S.sheet){el.innerHTML="";el.dataset.kind="";el.className="";lastStep=-1;return}
  const v=S.sheet==="pay"?payBody():S.sheet==="promo"?promoBody():S.sheet==="legal"?{title:"Legal documents",body:legalRows(),foot:""}:nodesBody(),fresh=el.dataset.kind!==S.sheet,step=S.sheet==="pay"?STEP[S.pay.step]+(S.pay.step==="fail"?.5:0):0;
  if(fresh){el.className="";el.dataset.kind=S.sheet;el.innerHTML=`<div class="scrim" data-act="sheet-close"></div><section class="sheet" role="dialog" aria-modal="true"><header><h2 class="title grow" style="font-size:24px;line-height:30px" id="sh-t"></h2><button class="btn quiet" data-act="sheet-close" aria-label="Close">${ic("x")}</button></header><div class="sbody" id="sh-b"></div><footer class="sfoot" id="sh-f"></footer></section>`}
  const b=$("#sh-b"),f=$("#sh-f"),dir=fresh||step===lastStep?"":step>lastStep?"sf":"sb";lastStep=step;
  $(".sheet").setAttribute("aria-label",v.title);
  const t=$("#sh-t");if(t.textContent!==v.title){t.innerHTML=`<span class="swap">${esc(v.title)}</span>`}
  b.className="sbody "+dir;f.className="sfoot "+dir;b.innerHTML=v.body;f.innerHTML=v.foot;f.hidden=!v.foot;
  if(S.pay?.busy)for(const control of $$("button,input,select",el))control.disabled=true;
  settle(el);S.just=null;
  if(fresh){const x=(S.sheet==="promo"?$("#promo-code"):null)||$("#sh-f .btn.primary")||$(".sheet button");x&&x.focus({preventScroll:true})}
}
function closeSheet(){const el=$("#sheet");if(!S.sheet||S.pay?.busy)return;stopPaymentPoll();S.sheet=null;if(S.pay?.step!=="wait")S.pay=null;if(reduced()){renderSheet();return}el.classList.add("closing");setTimeout(()=>{if(!S.sheet)renderSheet()},200)}
function loading(el,ms,fn){if(!el||el.classList.contains("loading"))return;el.classList.add("loading");setTimeout(fn,reduced()?0:ms)}

/* ── actions ──────────────────────────────────────────────────────────── */
const act={
  hub:()=>go(S.screen==="home"?"account":"home"),

  go:el=>go(el.dataset.v), toggle, "connect-here":()=>{go("home");setTimeout(()=>toggle().catch(report),reduced()?0:420)},

  theme:el=>{
    if(VAR[S.v].themes.length<2)return;const next=el.dataset.v||(root.dataset.theme==="dark"?"light":"dark");if(next===root.dataset.theme)return;
    const apply=()=>{root.dataset.theme=next;readTheme();if(S.screen==="settings")rr()};
    if(!document.startViewTransition||reduced()||document.hidden)return apply();
    const r=el.getBoundingClientRect(),x=r.left+r.width/2,y=r.top+r.height/2,R=Math.hypot(Math.max(x,innerWidth-x),Math.max(y,innerHeight-y));
    document.startViewTransition(apply).ready.then(()=>root.animate({clipPath:[`circle(0px at ${x}px ${y}px)`,`circle(${R}px at ${x}px ${y}px)`]},{duration:560,easing:"cubic-bezier(.4,0,.2,1)",pseudoElement:"::view-transition-new(root)"})).catch(()=>{})},

  "sheet-close":closeSheet,
  legal:async()=>{S.sheet="legal";renderSheet();await loadLegal();if(S.sheet==="legal")renderSheet()},
  "legal-retry":()=>loadLegal(true),
  "legal-doc":el=>{const doc=S.legal?.docs[+el.dataset.i];if(doc)return invoke("open_external",{url:doc.url})},
  "legal-link":async el=>{await loadLegal();const doc=S.legal?.docs.find(d=>d.key===el.dataset.key);if(doc)return invoke("open_external",{url:doc.url});S.sheet="legal";renderSheet()},
  "history-refresh":()=>loadHistory(),
  promo:()=>{if(!S.signed_in)return go("account");S.sheet="promo";renderSheet()},
  "go-devices":()=>{S.pendingSwitch=null;go("devices")},
  filter:el=>{S.filter=el.dataset.v;const s=el.parentElement,i=[...s.children].indexOf(el);s.dataset.i=i;s.style.setProperty("--i",i);mem.seg.filter=i;$$("button",s).forEach(b=>b.setAttribute("aria-pressed",b===el));syncConns(true)},
  pause:()=>{S.paused=!S.paused;rr()}, flush:()=>syncConns(true),
  quick:el=>{const c=S.conns.find(x=>x.id===+el.dataset.id);if(!c)return;
    if(c.origin==="user"&&c.rule.target===el.dataset.v){go("rules");toast("This rule already sends it "+TARGET[c.rule.target][0].toLowerCase()+".");return}
    S.scopeFor={id:c.id,target:el.dataset.v,i:0};connEls.get(c.id).innerHTML=rowHTML(c);$("#scopeadd").focus()},
  scope:el=>{S.scopeFor.i=+el.dataset.i;$$(".scope .chip").forEach(b=>b.setAttribute("aria-pressed",b===el))},
  "scope-cancel":()=>{const c=S.conns.find(x=>x.id===S.scopeFor.id);S.scopeFor=null;if(c)connEls.get(c.id).innerHTML=rowHTML(c)},
  "scope-add":()=>{const sc=S.scopeFor,c=S.conns.find(x=>x.id===sc.id),o=scopeOptions(c)[sc.i];S.scopeFor=null;
    const dup=S.rules.find(r=>r.type===o[0]&&r.value===o[1]);
    if(dup){dup.target=sc.target;dup.on=true;applyRules();toast(`Rule updated: ${o[1]} → ${TARGET[sc.target][0]}`)}else addRule(o[0],o[1],sc.target);
    syncConns(true)},
  "comp-mode":()=>{S.comp.raw=!S.comp.raw;S.comp.value="";rr()},
  "rule-type":el=>{const type=TYPES.find(t=>t[0]===el.dataset.v);if(!type)return;S.comp.type=type[0];$("#ctype-label").textContent=type[1];$("#cval").placeholder=type[3];$$(".rule-type-list button").forEach(b=>b.setAttribute("aria-pressed",b.dataset.v===type[0]));const picker=$("#ctype");picker.open=false;$("summary",picker).focus({preventScroll:true});showMsg()},
  "comp-target":el=>{S.comp.target=el.dataset.v;const s=el.parentElement,i=[...s.children].indexOf(el);s.dataset.i=i;s.style.setProperty("--i",i);mem.seg.ctarget=i;$$("button",s).forEach(b=>b.setAttribute("aria-pressed",b===el))},
  "rule-add":()=>{const c=S.comp,el=$("#cmsg");let type=c.type,value=c.value,target=c.target;const bad=t=>{el.className="msg";void el.offsetWidth;el.className="msg err";el.textContent=t};
    if(c.raw){const p=parseRaw(c.value);if(p.err)return bad(p.err);({type,value,target}=p)}
    const r=validate(type,value);if(r.err||!r.v)return bad(r.err||"Type a value first.");
    addRule(type,r.v,target,false);c.value="";rr();$(c.raw?"#rawv":"#cval").focus()},
  "rule-toggle":el=>{const r=S.rules.find(x=>x.id===+el.dataset.id);r.on=!r.on;applyRules();rr()},
  "rule-target":el=>{const r=S.rules.find(x=>x.id===+el.dataset.id),k=Object.keys(TARGET);r.target=k[(k.indexOf(r.target)+1)%k.length];applyRules();rr()},
  "rule-move":el=>{const i=S.rules.findIndex(x=>x.id===+el.dataset.id),j=i+ +el.dataset.d;if(j<0||j>=S.rules.length)return;[S.rules[i],S.rules[j]]=[S.rules[j],S.rules[i]];applyRules();rr();const b=$(`[data-act="rule-move"][data-id="${el.dataset.id}"][data-d="${el.dataset.d}"]`);b&&!b.disabled&&b.focus()},
  "rule-del":el=>{const id=+el.dataset.id;collapse($(`#rlist .rule[data-id="${id}"]`),()=>{const i=S.rules.findIndex(x=>x.id===id);if(i<0)return;const[r]=S.rules.splice(i,1);applyRules();render();toast(`Rule deleted: ${r.value}`,[["Undo",()=>{S.rules.splice(i,0,r);applyRules();if(S.screen==="rules")rr()}]])})},
  edit:el=>{S.edit=+el.dataset.id;rr()}, "edit-cancel":()=>{S.edit=null;rr()},
  "edit-save":el=>{const r=S.rules.find(x=>x.id===+el.dataset.id),v=validate(r.type,$("#editv").value,r.id);if(v.err||!v.v){toast(v.err||"A rule needs a value.");return}r.value=v.v;S.edit=null;applyRules();rr()},
  switch:el=>{S.pendingSwitch=+el.dataset.id;rr()}, "switch-cancel":()=>{S.pendingSwitch=null;rr()},

  "dev-edit":el=>{if(S.deviceBusy)return;S.edit="d"+el.dataset.id;rr()}, "dev-ask":el=>{if(S.deviceBusy)return;S.edit="x"+el.dataset.id;rr()},

};
document.addEventListener("click",e=>{for(const picker of $$(".rule-types[open]"))if(!picker.contains(e.target))picker.open=false;const el=e.target.closest("[data-act]");if(el?.tagName==="A")e.preventDefault();if(el&&!el.disabled&&act[el.dataset.act])Promise.resolve().then(()=>{if(S.pay?.busy&&el.closest("#sheet"))return;return act[el.dataset.act](el,e)}).catch(report)});
document.addEventListener("input",e=>{const t=e.target;if(t.id==="cval"||t.id==="rawv"){S.comp.value=t.value;showMsg()}if(t.id==="q"){S.q=t.value;syncConns(true)}});
document.addEventListener("keydown",e=>{
  const t=e.target;
  const picker=$(".rule-types[open]");
  if(picker&&e.key==="Escape"){e.preventDefault();picker.open=false;$("summary",picker).focus();return}
  if(picker&&picker.contains(t)&&["ArrowDown","ArrowUp","Home","End"].includes(e.key)){e.preventDefault();const options=$$(".rule-type-list button",picker),i=options.indexOf(t),next=e.key==="Home"?0:e.key==="End"?options.length-1:e.key==="ArrowDown"?(i+1)%options.length:(i<=0?options.length:i)-1;options[next]?.focus();return}
  if(e.key==="Tab"&&S.sheet){const all=$$('.sheet button:not([disabled]),.sheet input,.sheet select').filter(el=>el.offsetParent);const first=all[0],last=all[all.length-1];if(e.shiftKey&&document.activeElement===first){e.preventDefault();last?.focus()}else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first?.focus()}}
  if(e.key==="Escape"){if(S.sheet)closeSheet();else if(S.scopeFor)act["scope-cancel"]();else if(S.edit){S.edit=null;rr()}}
  if(e.key==="Enter"){if(t.id==="cval"||t.id==="rawv")act["rule-add"]();if(t.id==="editv")$('[data-act="edit-save"]').click();if(t.id==="devname")saveDevName(t).catch(report)}
  if(e.altKey&&(e.key==="ArrowUp"||e.key==="ArrowDown")){const r=t.closest(".rule[data-id]");if(r){e.preventDefault();act["rule-move"]({dataset:{id:r.dataset.id,d:e.key==="ArrowUp"?"-1":"1"}})}}
});
document.addEventListener("focusout",e=>{if(e.target.id==="devname"&&S.edit)saveDevName(e.target).catch(report)});
let dragId=null;
document.addEventListener("dragstart",e=>{const r=e.target.closest&&e.target.closest("#rlist .rule[data-id]");if(!r)return;dragId=+r.dataset.id;r.classList.add("dragging");e.dataTransfer.effectAllowed="move"});
document.addEventListener("dragover",e=>{const r=e.target.closest&&e.target.closest("#rlist .rule[data-id]");if(!r||dragId==null)return;e.preventDefault();$$("#rlist .over-top").forEach(x=>x.classList.remove("over-top"));r.classList.add("over-top")});
document.addEventListener("drop",e=>{const r=e.target.closest&&e.target.closest("#rlist .rule[data-id]");if(!r||dragId==null)return;e.preventDefault();if(+r.dataset.id===dragId){dragId=null;return}const from=S.rules.findIndex(x=>x.id===dragId),[m]=S.rules.splice(from,1);S.rules.splice(S.rules.findIndex(x=>x.id===+r.dataset.id),0,m);dragId=null;applyRules();rr()});
document.addEventListener("dragend",()=>{dragId=null;$$(".dragging,.over-top").forEach(x=>x.classList.remove("dragging","over-top"))});
addEventListener("resize",markNav);



/* ── Native engine and cabinet: no credentials leave the Rust bridge ───── */
const invoke=(command,args)=>window.__TAURI__.core.invoke(command,args);
const api=(method,path,body=null)=>invoke("cabinet_request",{method,path,body});
function friendlyError(error){
 if(/device identification/i.test(String(error?.message||error)))return "Device identification is unavailable. Restart Windows and try again.";
 const text=String(error?.message||error||'');
 if(/wrong.*(email|password)|incorrect.*(email|password)|invalid credentials|неверн.*парол/i.test(text))return 'Wrong email or password.';
 if(/sign in again|session expired|HTTP 401|not signed in/i.test(text))return 'Please sign in again.';
 if(/already registered|email.*registered|email уже/i.test(text))return 'This email is already registered. Try signing in.';
 if(/code.*expired|link.*expired|устарел|истек/i.test(text))return 'This code or reset link has expired. Request a new one.';
 if(/code.*incorrect|incorrect.*code|six-digit|неверн.*код/i.test(text))return 'That code is incorrect. Try again.';
 if(/valid email/i.test(text))return 'Enter a valid email address.';
 if(/password.*8.*128/i.test(text))return 'Use a password between 8 and 128 characters.';
 if(/accept.*agreement|accept.*terms|consent/i.test(text))return 'Please accept both agreements to create an account.';
 if(/too many|rate.limit|HTTP 429/i.test(text))return 'Too many attempts. Please wait and try again.';
 if(/insufficient balance|недостаточно/i.test(text))return 'Not enough balance. Top up or choose another payment method.';
 if(/payment.*(not confirmed|already submitted)|check.*cabinet|uncertain/i.test(text))return 'Payment is not confirmed. Check it in your cabinet before trying again.';
 if(/no_subscription|add a subscription|choose a plan|missing subscription/i.test(text))return 'Choose a plan before connecting.';
 if(/disconnect before/i.test(text))return 'Disconnect first, then try again.';
 if(/administrator|admin_required/i.test(text))return 'Administrator access is needed for whole-device mode.';
 if(/unlimited|maximum device/i.test(text))return 'This plan already has the maximum allowance.';
 if(/network|connection failed|could not reach|internet/i.test(text))return 'Could not connect. Check your internet and try again.';
 return 'Something went wrong. Try again later.';
}
function report(error){const raw=String(error?.message||error);toast(friendlyError(error));if(/HTTP 401|not signed in|sign in again/i.test(raw)){S.signed_in=false;S.me=null;S.subs=[];S.devices={};S.deviceLoaded={};S.balance=0;stopPaymentPoll();S.pay=null;S.sheet=null;go('account');renderSheet();if(S.screen==='account')rr()}}

const accountName=()=>S.me?.email||S.me?.first_name||S.me?.username||"Your account";
function engineBoot(data){
  S.has_sub=data.has_sub;S.closeToTray=data.close_to_tray!==false;S.mode=data.mode||"tun";S.node=data.node||"";S.hwid=data.hwid||"";S.hwidAvailable=data.hwid_available!==false;S.engine_sub_id=data.sub_id??null;S.version=data.version||"";S.profile=data.profile||{};
  if(data.sub_id)S.activeSub=data.sub_id;
  S.rules=(data.rules||[]).map(r=>({...r}));savedRules=S.rules.map(r=>({...r}));uid=Math.max(uid,...S.rules.map(r=>r.id));
  setConn(data.connected?"connected":"idle");
}
function applyMe(me){
  if(S.me?.id!==me.id){S.transactions=null;S.historyError="";S.historyLoading=false;historyGeneration++}
  S.me=me;S.signed_in=true;S.balance=me.balance;
  METHODS.splice(1,METHODS.length,...(me.payment_methods||[]).map(m=>({id:m.id,name:m.name,fee:m.fee_percent,note:"Open secure payment in your browser"})));
  S.subs=(me.subscriptions||[]).map(s=>({...s,tariff:s.tariff_id,days:s.days_left,term:s.term_days||Math.max(1,s.days_left),gb:s.traffic_gb,used:s.used_gb,expires:s.expires_at?new Date(s.expires_at).toLocaleDateString("en-GB",{day:"numeric",month:"long"}):"—"}));
  if(!sub())S.activeSub=S.subs.find(s=>s.id===S.engine_sub_id)?.id??S.subs.find(s=>s.status==="active")?.id??S.subs[0]?.id??null;
  S.subs.forEach(s=>S.devices[s.id]??=[]);
}
async function loadLegal(force=false){
  if(S.legalLoading||S.legal&&!force)return;
  S.legalLoading=true;S.legalError="";
  try{
    const result=await api("GET","/legal");
    if(!Array.isArray(result.docs)||result.docs.length>32)throw Error("invalid documents");
    const docs=result.docs.map(doc=>{const url=new URL(doc.url);if(url.protocol!=="https:"||url.username||url.password||typeof doc.title!=="string"||doc.title.length>300)throw Error("invalid document");return{key:String(doc.key||""),title:doc.title,url:url.href}});
    S.legal={docs,version:String(result.version||"").slice(0,128)};
  }catch(error){S.legalError=friendlyError(error)}finally{S.legalLoading=false;if(S.sheet==="legal")renderSheet();paintScreen("settings")}
}
function legalRows(){
  if(S.legalError)return `<div class="pad col"><p class="cap">${esc(S.legalError)}</p><button class="btn" data-act="legal-retry">Try again</button></div>`;
  if(!S.legal)return '<div class="pad cap dim">Loading documents…</div>';
  return S.legal.docs.map((doc,i)=>`<button class="set navrow" data-act="legal-doc" data-i="${i}">${ic("info")}<span class="grow set-t">${esc(doc.title)}</span>${ic("ext","s")}</button>`).join("")+`<div class="pad cap dim">Version ${esc(S.legal.version)}</div>`;
}
async function loadHistory(){
  if(!S.signed_in||!S.me||S.historyLoading)return;
  const account=S.me.id,generation=++historyGeneration;
  S.historyLoading=true;S.historyError="";paintScreen("balance");
  try{
    const result=await api("GET","/me/transactions");
    if(!Array.isArray(result.transactions)||result.transactions.some(t=>!Number.isSafeInteger(t.amount_kopecks)||typeof t.type!=="string"))throw Error("invalid history");
    if(generation===historyGeneration&&S.signed_in&&S.me?.id===account)S.transactions=result.transactions;
  }catch(error){if(generation===historyGeneration&&S.signed_in&&S.me?.id===account){S.historyError=friendlyError(error);report(error)}}
  finally{if(generation===historyGeneration){S.historyLoading=false;paintScreen("balance")}}
}
function historyCard(){
  const labels={topup:"Balance top-up",purchase:"Plan purchase",renew:"Plan renewal",extra_devices:"Device slots",extra_gb:"Extra traffic",promo:"Promo code",referral:"Referral bonus",referral_bonus:"Referral bonus",referral_join:"Referral bonus",referral_refund:"Referral adjustment",refund:"Refund",admin_balance:"Balance adjustment",admin_grant:"Subscription grant"};
  const statuses={completed:"Completed",pending:"Pending",failed:"Failed",error:"Failed",cancelled:"Cancelled",canceled:"Cancelled",refunded:"Refunded"};
  const rows=(S.transactions||[]).map(t=>{const amount=t.amount_kopecks,date=new Date(t.created_at),label=labels[t.type]||t.description||"Balance adjustment",status=statuses[t.status]||"Recorded";
    return `<div class="transaction" role="listitem"><span class="cc">${ic(amount>0?"plus":"wallet","s")}</span><div class="grow"><div class="set-t">${esc(label)}</div><div class="cap dim">${Number.isFinite(date.getTime())?esc(date.toLocaleString()):"—"}${t.description&&t.description!==label?" · "+esc(t.description):""}</div></div><div class="transaction-end"><div class="mono ${amount>0&&t.status==="completed"?"credit":""}">${amount>0?"+":amount<0?"−":""}${rub(Math.abs(amount))}</div><span class="cap dim">${status}</span></div></div>`}).join("");
  return `<section class="card clip history-card"><header class="pad row"><div class="grow"><h2 class="h">Transaction history</h2><span class="cap dim">Latest 50 transactions from your cabinet</span></div><button class="btn" data-act="history-refresh" ${S.historyLoading?"disabled":""}>${S.historyLoading?"Refreshing…":"Refresh"}</button></header>${S.historyError?`<div class="pad cap">${esc(S.historyError)}</div>`:""}<div class="list" role="list">${rows||`<div class="empty"><p>${S.historyLoading||S.transactions===null&&!S.historyError?"Loading history…":S.historyError?"History could not be loaded.":"No transactions yet."}</p></div>`}</div></section>`;
}
let refreshPromise=null;
async function refreshMe(){
  if(!S.signed_in)return;
  if(!refreshPromise)refreshPromise=api("GET","/me").then(me=>{applyMe(me);return me}).finally(()=>refreshPromise=null);
  return refreshPromise;
}
async function loadDevices(id){
  if(!id)return;
  const result=await api("GET",`/devices?sub_id=${id}`);S.deviceLoaded[id]=true;
  S.devices[id]=(result.devices||[]).map(d=>({...d,name:d.name||d.platform||d.os||"Device",meta:[d.platform,d.os].filter(Boolean).join(" · "),kind:/ios|android|phone/i.test(d.platform||d.os||"")?"phone":"laptop",me:d.hwid===S.hwid,added:d.created_at?new Date(d.created_at).toLocaleDateString("en-GB"):""}));
}
async function loadTariffs(){
  const result=await api("GET","/tariffs");
  TARIFFS.splice(0,TARIFFS.length,...result.tariffs.map(t=>({...t,price:t.price_kopecks,gb:t.traffic_gb})));
}
function paintScreen(screen){if(S.screen!==screen||S.edit)return;const m=$("#main");if(m.classList.contains("enter")){pendingPaint=screen;return}if(!m.classList.contains("leave"))render()}
async function refreshScreen(screen){
  if(screen==="settings")await loadLegal();
  if(!S.signed_in)return;
  await refreshMe();
  if(screen==="subs")await Promise.all(S.subs.map(s=>loadDevices(s.id)));
  if(screen==="devices"||screen==="account")await loadDevices(S.activeSub);
  if(screen==="referral")S.referral=await api("GET","/referral");
  if(screen==="balance")await loadHistory();
  paintScreen(screen);
}
async function loadNodes(){
  if(!S.has_sub)return;
  applyNodes(await invoke("proxies"));
}
function applyNodes(result){
  S.node=result.now||S.node;
  const previous=new Map(NODES.map(n=>[n.id,n]));
  NODES.splice(0,NODES.length,...result.nodes.map(n=>{const old=previous.get(n.name),unknown=n.status==="unknown";return{id:n.name,name:nodeLabel(n.name),sub:n.now?"Using "+nodeLabel(n.now):"From your subscription",status:unknown&&old?old.status:n.status|| (n.delay>0?"ok":"unknown"),ms:n.delay>0?n.delay:unknown&&old?old.ms:null}}));
  if(S.sheet==="nodes")renderSheet();
  if(S.screen==="home"){renderSide();renderStatus()}
}
async function useSubscription(id){
  if(S.deviceBusy)return;
  const reconnect=S.conn==="connected";
  if(reconnect){await invoke("disconnect");setConn("idle")}
  try{engineBoot(await invoke("cabinet_use_subscription",{subId:id}));S.activeSub=id;S.pendingSwitch=null;S.just="use"+id;rr();if(reconnect)await toggle()}
  catch(error){S.pendingSwitch=null;rr();throw error}
}
async function watchTraffic(){await invoke("watch",{on:S.screen==="conns"&&S.conn==="connected"&&!S.paused&&!document.hidden})}
function legalLink(key,label){const doc=S.legal?.docs.find(d=>d.key===key);return `<a class="legal-link" href="${esc(doc?.url||'#')}" data-act="legal-link" data-key="${key}">${label}</a>`}
function signInScreen(){
 const mode=S.auth.mode,email=`<label for="email">Email</label><input class="input" id="email" type="email" autocomplete="email" value="${esc(S.auth.email)}" required>`,password=`<label for="password">${mode==='reset'?'New password':'Password'}</label><input class="input" id="password" type="password" minlength="8" maxlength="128" autocomplete="${mode==='login'?'current-password':'new-password'}" required>`;
 const title={login:'Sign in',register:'Create account',verify:'Check your email',forgot:'Reset password',reset:'Set a new password'}[mode];
 let fields=mode==='verify'?`<p class="cap">Enter the six-digit code sent to ${esc(S.auth.email)}.</p><label for="code">Email code</label><input class="input mono" id="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9]{6}" maxlength="6" required>`:mode==='reset'?`<p class="cap">If this email belongs to an account, we sent a reset link. Copy that link from your email and paste it here; you do not need to open the website.</p><label for="reset-link">Reset link or token</label><input class="input" id="reset-link" type="text" autocomplete="off" spellcheck="false" required>${password}`:email+(mode==='forgot'?'':password);
 if(mode==='register')fields+=`<label class="consent"><input type="checkbox" id="terms" required><span>I accept the ${legalLink('offer','public offer')} and ${legalLink('terms','terms of service')}.</span></label><label class="consent"><input type="checkbox" id="personal-data" required><span>I consent to ${legalLink('consent','processing of my personal data')} and have read the ${legalLink('privacy','privacy policy')}.</span></label>`;
 const button={login:'Sign in',register:'Create account',verify:'Verify email',forgot:'Send reset link',reset:'Save password'}[mode];
 return `<form id="${mode}" class="card pad auth"><h1 class="h">${title}</h1>${fields}<button class="btn primary block" style="margin-top:20px" type="submit" ${S.authBusy?'disabled':''}>${button}</button>${mode==='login'?'<button class="btn quiet block" type="button" data-act="auth" data-v="register">Create account</button><button class="btn quiet block" type="button" data-act="auth" data-v="forgot">Forgot password?</button>':'<button class="btn quiet block" type="button" data-act="auth" data-v="login">Back to sign in</button>'}${mode==='verify'?`<button class="btn quiet block" type="button" data-act="auth-resend" ${Date.now()<S.auth.retryAt?'disabled':''}>Resend code</button>`:''}<button class="btn quiet block" type="button" data-act="legal">Legal documents</button></form>`;
}

function subscriptionInfo(){
 const p=S.profile||{},used=(p.upload||0)+(p.download||0),gb=bytes=>(bytes/1073741824).toLocaleString('en',{maximumFractionDigits:2})+' GB';
 const date=seconds=>{const value=new Date(seconds*1000);return Number.isFinite(value.getTime())?value.toLocaleString():'—'};
 return `<section class="card pad col"><div class="row"><h2 class="h grow">${esc(p.title||'Subscription')}</h2><button class="btn" data-act="subscription-refresh" ${!S.has_sub||S.refreshing?'disabled':''}>${S.refreshing?'Updating…':'Update subscription'}</button></div>${p.updated_at?`<div class="cap dim">Updated ${date(p.updated_at)}</div>`:''}${p.total!=null?`<div class="cap">${gb(used)} used · ${p.total>0?gb(p.total)+' total':'Unlimited traffic'}</div>`:''}${p.expires!=null?`<div class="cap">${p.expires>0?'Expires '+date(p.expires):'No expiry date'}</div>`:''}${p.update_hours?`<div class="cap dim">Profile refresh interval: ${p.update_hours} ${p.update_hours===1?'hour':'hours'}</div>`:''}<div class="row">${p.support_url?'<button class="btn quiet" data-act="profile-link" data-v="support_url">Support</button>':''}${p.web_page_url?'<button class="btn quiet" data-act="profile-link" data-v="web_page_url">Subscription details</button>':''}</div></section>`;
}
act['subscription-refresh']=async()=>{if(S.refreshing||S.testing||!S.has_sub)return;S.refreshing=true;if(S.sheet==="nodes")renderSheet();if(S.screen==='settings'||S.screen==='subs')rr();try{engineBoot(await invoke('refresh_subscription'));await loadNodes();toast('Subscription updated.')}catch(error){report(error)}finally{S.refreshing=false;if(S.sheet==="nodes")renderSheet();if(S.screen==='settings'||S.screen==='subs')rr()}};
act['close-tray']=async()=>{const on=!S.closeToTray;await invoke('set_close_to_tray',{on});S.closeToTray=on;rr()};
act['profile-link']=el=>{const url=S.profile?.[el.dataset.v];if(url)return invoke('open_external',{url})};
let nameSaving=false;
async function saveDevName(input){
  if(nameSaving||S.edit!=="d"+input.dataset.id)return;
  const name=input.value.trim();if(!name){toast("A device needs a name.");return}
  nameSaving=true;
  try{await api("PATCH",`/devices/${encodeURIComponent(input.dataset.id)}?sub_id=${S.activeSub}`,{name});S.edit=null;await loadDevices(S.activeSub);rr()}
  finally{nameSaving=false}
}

/* ── Checkout uses the cabinet's integer pricing and idempotency contract ── */
const tickEl=`<span class="tick">${ic("check")}</span>`;
function quote(){
  const p=S.pay,per=PERIODS.find(x=>x.days===p.days)||PERIODS[0],t=TARIFFS.find(x=>x.id===p.tariff)||p.sub||{},m=METHODS.find(x=>x.id===p.method)||METHODS[0];
  const months=p.sub?.addon_months_remaining??Math.max(1,Math.ceil((p.sub?.days||0)/30));
  let list=p.kind==="topup"?p.amount:p.kind==="slot"?p.count*p.sub.extra_device_price_kopecks*months:p.kind==="traffic"?p.count*p.sub.extra_gb_price_kopecks*months:Math.trunc((p.kind==="renew"?(p.sub.renew_price_per_30_kopecks??p.sub.price_per_30_kopecks):t.price)*per.mult);
  const discount=["buy","renew"].includes(p.kind)?Math.min(100,Math.max(0,S.me?.discount?.percent||0)):0;
  const net=Math.max(0,list-Math.floor(list*discount/100));
  return{t,per,m,list,net,total:Math.trunc(net*(1+m.fee/100)),months};
}
function fixMethod(){
  if(S.pay.method==="balance"&&(S.pay.kind==="topup"||quote().net>S.balance))S.pay.method=METHODS[1]?.id||"balance";
}
async function openPay(kind,subId){
  if(S.pay?.busy)return;
  if(!S.signed_in){go("account");return}
  if(S.pay?.step==="wait"){S.sheet="pay";renderSheet();schedulePaymentPoll();return}
  await Promise.all([refreshMe(),loadTariffs()]);
  const s=S.subs.find(x=>x.id===(subId||S.activeSub));
  if(kind!=="buy"&&kind!=="topup"&&!s)throw Error("Choose a plan first.");
  if(kind==="renew"&&!s.tariff)kind="buy";
  if(kind==="buy"&&!TARIFFS.length)throw Error("No plans are available right now.");
  if(kind==="slot"&&(!(s.devices>0)||S.me.max_devices&&s.devices>=S.me.max_devices))throw Error("This plan already has the maximum device slots.");
  if(kind==="traffic"&&(!(s.gb>0)||s.gb%50))throw Error(s.gb?"Open the cabinet to change this traffic limit.":"This plan already has unlimited traffic.");
  S.pay={kind,step:kind==="buy"?"plan":"method",tariff:kind==="buy"?TARIFFS[0].id:s?.tariff,days:30,method:"balance",sub:s,count:1,amount:10000,busy:false,error:""};
  fixMethod();delete mem.n.total;delete mem.seg.period;S.sheet="pay";renderSheet();
}
function payBody(){
  const p=S.pay,q=quote(),enough=S.balance>=q.net;
  const periods=`<div><div class="over" style="margin-bottom:8px">Period</div>${seg("period",PERIODS.map(x=>[x.days,x.label]),p.days,"pay-days","wide")}<div class="cap" style="margin-top:8px;min-height:16px;color:var(--ok)">${q.per.mo>1?`${q.per.label} · period discount included`:""}</div></div>`;
  const total=`<div class="total"><span class="cap">Total${q.m.fee?` incl. ${q.m.fee}% fee`:""}</span><span class="num">${num("total",q.total,"rub")}</span></div>`;
  const err=p.error?`<div class="cap pay-error" role="alert">${esc(p.error)}</div>`:"";
  if(p.step==="plan")return{title:"Choose a plan",body:`${TARIFFS.map(t=>`<button class="opt ${S.just==="t"+t.id?"just":""}" data-act="pay-tariff" data-v="${t.id}" aria-pressed="${p.tariff===t.id}">${tickEl}<span class="grow"><span style="font-weight:600;display:block">${esc(t.name)}</span><span class="cap dim">${t.devices||"Unlimited"} devices · ${t.gb?t.gb+" GB a month":"unlimited traffic"}</span></span><span class="mono">${rub(t.price)}<span class="dim">/mo</span></span></button>`).join("")}${periods}`,foot:`${total}<button class="btn primary block" data-act="pay-step" data-v="method">Continue</button>`};
  if(p.step==="method")return{title:p.kind==="slot"?"Add device slots":p.kind==="traffic"?"Add traffic":p.kind==="renew"?`Renew ${p.sub.plan}`:p.kind==="topup"?"Top up balance":"Payment method",body:`${p.kind==="renew"?periods:""}${["slot","traffic"].includes(p.kind)?`<div class="card pad"><div class="cap">${esc(p.sub.plan)} · until ${esc(p.sub.expires)}</div><div class="row" style="margin-top:12px"><button class="btn" data-act="pay-count" data-v="-1" aria-label="Less" ${p.count<=1?"disabled":""}>−</button><span class="num grow" style="text-align:center">${num("addon",p.count*(p.kind==="traffic"?50:1))}<small>${p.kind==="traffic"?"GB":"slots"}</small></span><button class="btn" data-act="pay-count" data-v="1" aria-label="More">+</button></div><div class="cap dim" style="margin-top:8px">For the remaining ${q.months} billing ${q.months===1?"month":"months"} of this plan.</div></div>`:""}${p.kind==="topup"?`<label class="cap" for="topup-amount">Amount, ₽ (100–100 000)</label><input class="input" type="number" id="topup-amount" value="${p.amount/100}" min="100" max="100000" step="1">`:""}<div><div class="over" style="margin-bottom:8px">Pay with</div><div class="col" style="gap:8px">${METHODS.filter(m=>p.kind!=="topup"||m.id!=="balance").map(m=>{const dis=m.id==="balance"&&!enough;return`<button class="opt ${S.just==="m"+m.id?"just":""}" data-act="pay-method" data-v="${m.id}" aria-pressed="${p.method===m.id}" ${dis?"disabled":""}>${tickEl}<span class="grow"><span style="font-weight:600;display:block">${esc(m.name)}</span><span class="cap dim">${m.id==="balance"?(dis?`Covers ${rub(S.balance)} of ${rub(q.net)}`:`${rub(S.balance)} available`):esc(m.note)}</span></span>${m.fee?`<span class="cap dim">+${m.fee}% fee</span>`:""}</button>`}).join("")}</div></div>${err}`,foot:`${total}<button class="btn primary block ${p.busy?"loading":""}" data-act="pay-go" ${p.busy||!Number.isSafeInteger(q.total)||q.total<=0||p.kind==="topup"&&p.method==="balance"?"disabled":""}>Pay ${rub(q.total)}</button>${p.kind==="buy"?'<button class="btn quiet block" data-act="pay-step" data-v="plan">Back</button>':""}`};
  if(p.step==="wait")return{title:"Waiting for payment",body:`<div class="wait"><div class="spin" aria-hidden="true"></div><div><div class="h">${p.link?"Finish the payment in your browser":"Waiting for confirmation"}</div><div class="cap" style="margin-top:4px">${esc(p.amountFormatted||rub(q.total))} · ${esc(q.m.name)}</div></div><div class="col" style="gap:8px;width:100%">${p.link?`<button class="btn block" data-act="pay-open">${ic("ext","s")}Open payment page again</button>`:""}<button class="btn block" data-act="cabinet">Check payment in cabinet</button><button class="btn block ${p.checking?"loading":""}" data-act="pay-check" ${p.checking?"disabled":""}>I paid, check now</button></div>${err}<div class="cap dim">Your plan updates after the payment provider confirms. You can close this panel and return to it from Plans.</div></div>`,foot:""};
  if(p.step==="done")return{title:"Done",body:`<div class="wait"><div class="mark" style="--c:var(--ok)"><svg viewBox="0 0 64 64"><circle cx="32" cy="32" r="29" pathLength="100"/><path d="M20 33l8.5 8.5L45 24" pathLength="100"/></svg><i></i><i></i></div><div><div class="h">${p.kind==="topup"?"Balance updated":"Your purchase is confirmed"}</div><div class="cap" style="margin-top:4px">${p.amountFormatted?esc(p.amountFormatted)+" · ":""}Your account details are up to date.</div></div></div>`,foot:'<button class="btn primary block" data-act="sheet-close">Done</button>'};
  return{title:"Payment not completed",body:`<div class="wait"><div class="mark bad" style="--c:var(--bad)"><svg viewBox="0 0 64 64"><circle cx="32" cy="32" r="29" pathLength="100"/><path d="M23 23l18 18M41 23L23 41" pathLength="100"/></svg></div><div class="h">${esc(p.error||"Payment is not confirmed.")}</div><div class="cap">${p.request?"Retry uses the same checkout request.":"You can start a new checkout."}</div></div>`,foot:p.kind==="topup"?'<button class="btn block" data-act="cabinet">Check payment in cabinet</button>':'<button class="btn primary block" data-act="pay-go">Try again</button><button class="btn block" data-act="pay-step" data-v="method">Choose another method</button>'};
}
function checkoutBody(p){
  const body={payment_method:p.method};
  if(p.kind==="buy")return{path:"/buy",body:{...body,tariff_id:p.tariff,days:p.days}};
  if(p.kind==="topup")return{path:"/topup",body:{...body,amount_kopecks:p.amount}};
  const path=`/subscription/${p.sub.id}/`;
  return p.kind==="renew"?{path:path+"renew",body:{...body,days:p.days}}:p.kind==="slot"?{path:path+"extra-devices",body:{...body,extra_count:p.count}}:{path:path+"extra-gb",body:{...body,new_total_gb:p.sub.gb+p.count*50}};
}
const checkoutPrefix="moonlight-checkout:";
function claimCheckout(path,body,p){
  const key=checkoutPrefix+JSON.stringify([S.me.id,path,body]);
  const existing=localStorage.getItem(key);
  if(existing){const record=JSON.parse(existing);if(!/^[0-9a-f-]{36}$/i.test(record.id))throw Error("Invalid checkout recovery data.");return{...record,key}}
  if(Object.keys(localStorage).filter(k=>k.startsWith(checkoutPrefix)).length>=128)throw Error("Too many unresolved checkouts. Check your account in the cabinet.");
  const record={id:crypto.randomUUID(),created:Date.now(),baseline:{balance:S.balance,ids:S.subs.map(s=>s.id),expires:p.sub?.expires_at,devices:p.sub?.devices,gb:p.sub?.gb}};
  localStorage.setItem(key,JSON.stringify(record));return{...record,key};
}
let paymentTimer=0;
function stopPaymentPoll(){clearTimeout(paymentTimer);paymentTimer=0}
function schedulePaymentPoll(){
  stopPaymentPoll();const p=S.pay;if(!p||p.step!=="wait"||p.kind!=="buy"||S.sheet!=="pay"||document.hidden||Date.now()-p.request.created>=1800000)return;
  paymentTimer=setTimeout(()=>checkPayment().catch(report),Date.now()-p.request.created<120000?3000:10000);
}
async function finishPayment(p,result){
  await refreshMe();if(S.pay!==p)return;
  p.step="done";p.error="";p.amountFormatted=result?.total_formatted||result?.amount_formatted||p.amountFormatted;
  if(result?.subscription_id)S.activeSub=result.subscription_id;
  if(p.request?.key)localStorage.removeItem(p.request.key);stopPaymentPoll();renderSheet();paintScreen(S.screen);
}
async function submitPayment(){
  const p=S.pay;if(!p||p.busy)return;
  const q=quote();if(!Number.isSafeInteger(q.total)||q.total<=0)throw Error("Choose a valid amount.");
  if(p.kind==="topup"&&(p.amount<10000||p.amount>10000000))throw Error("Top-up must be between 100 and 100 000 ₽.");
  const submitted=structuredClone(p),{path,body}=checkoutBody(submitted);
  p.busy=true;p.error="";renderSheet();
  try{
    p.request=claimCheckout(path,body,submitted);
    // /topup has no server idempotency key: never repeat an uncertain submission.
    if(p.kind==="topup"&&p.request.sent)throw Error("This top-up was already submitted. Check it in the cabinet.");
    if(p.kind==="topup"){p.request.sent=true;localStorage.setItem(p.request.key,JSON.stringify(p.request))}
    const result=await api("POST",path,p.kind==="topup"?body:{...body,request_id:p.request.id});
    if(S.pay!==p)return;
    if(!result.ok)throw Error("The service did not accept the payment.");
    p.amountFormatted=result.total_formatted||result.amount_formatted;
    if(["compensated","failed","expired","refunded","canceled"].includes(result.status)){localStorage.removeItem(p.request.key);p.request=null;throw Error(result.status==="compensated"?"The purchase could not be completed. Your balance was restored.":`Payment status: ${result.status}`)}
    if(result.status==="completed"||body.payment_method==="balance"&&result.status!=="pending"){await finishPayment(p,result);return}
    p.link=result.link||null;p.step="wait";renderSheet();schedulePaymentPoll();if(p.link)await invoke("open_external",{url:p.link});
  }catch(error){if(S.pay===p){p.error=friendlyError(error);p.step="fail";renderSheet()}}
  finally{p.busy=false;if(S.pay===p&&S.sheet==="pay")renderSheet()}
}
async function checkPayment(){
  const p=S.pay;if(!p||p.step!=="wait"||p.checking)return;
  p.checking=true;p.error="";if(S.sheet==="pay")renderSheet();
  try{
    let result=null;
    if(p.kind==="buy")result=await api("GET",`/checkout/${encodeURIComponent(p.request.id)}`);
    await refreshMe();if(S.pay!==p)return;
    if(result?.status==="completed"){await finishPayment(p,result);return}
    if(["compensated","failed","expired","refunded","canceled"].includes(result?.status)){localStorage.removeItem(p.request.key);p.request=null;p.step="fail";p.error=`Payment status: ${result.status}`}
    else if(p.kind!=="buy")p.error="Account details refreshed. Check this payment in your cabinet.";
    else if(Date.now()-p.request.created>=1800000)p.error="Still not confirmed. Check your account in the cabinet before creating another payment.";
    else p.error="Payment has not been confirmed yet.";
    if(S.sheet==="pay")renderSheet();paintScreen(S.screen);
  }catch(error){p.error=friendlyError(error);if(S.pay===p&&S.sheet==="pay")renderSheet()}
  finally{p.checking=false;if(S.pay===p&&S.sheet==="pay")renderSheet();schedulePaymentPoll()}
}
Object.assign(act,{
  signin:()=>go("account"),"account-refresh":()=>refreshScreen(S.screen),cabinet:()=>invoke("open_external",{url:S.cabinet.cabinet_url}),
  logout:async()=>{if(S.conn==="connected"){await invoke("disconnect");setConn("idle")}await invoke("cabinet_logout");engineBoot(await invoke("boot"));stopPaymentPoll();S.pay=null;S.sheet=null;S.signed_in=false;S.me=null;S.subs=[];S.devices={};S.deviceLoaded={};S.balance=0;S.referral=null;S.activeSub=null;go("account");renderSheet();rr()},
  mode:async el=>{if(S.mode===el.dataset.v)return;if(S.conn!=="idle"||S.busy)throw Error("Disconnect before changing mode.");await invoke("set_mode",{mode:el.dataset.v});S.mode=el.dataset.v;S.screen==="home"?renderSide():rr();renderBar()},
  nodes:async()=>{if(!S.has_sub){toast("Choose a subscription to load its servers.");return}await loadNodes();S.sheet="nodes";renderSheet()},
  node:async el=>{await invoke("select_node",{name:el.dataset.v});S.node=el.dataset.v;S.just="n"+S.node;renderSheet();setTimeout(()=>{closeSheet();S.screen==="home"?(renderSide(),renderStatus()):rr();renderBar()},reduced()?0:320)},
  test:async()=>{if(S.testing||S.refreshing)return;S.testing=true;renderSheet();try{applyNodes(await invoke("test_latency"))}finally{S.testing=false;if(S.sheet==="nodes")renderSheet()}},
  pause:async()=>{S.paused=!S.paused;await watchTraffic();rr()},
  "switch-ok":el=>useSubscription(+el.dataset.id),
  "pay-buy":()=>openPay("buy"),"pay-renew":el=>openPay("renew",+el.dataset.id||undefined),"pay-slot":()=>openPay("slot"),"pay-traffic":el=>openPay("traffic",+el.dataset.id),"pay-topup":()=>openPay("topup"),
  "pay-tariff":el=>{if(S.pay?.busy)return;S.pay.tariff=+el.dataset.v;S.just="t"+S.pay.tariff;fixMethod();renderSheet()},
  "pay-days":el=>{if(S.pay?.busy)return;S.pay.days=+el.dataset.v;fixMethod();renderSheet()},
  "pay-method":el=>{if(S.pay?.busy)return;S.pay.method=el.dataset.v;S.just="m"+S.pay.method;renderSheet()},
  "pay-step":el=>{if(S.pay?.busy)return;S.pay.step=el.dataset.v;S.pay.error="";fixMethod();renderSheet()},
  "pay-count":el=>{if(S.pay?.busy)return;const p=S.pay,next=Math.max(1,p.count+ +el.dataset.v);if(p.kind==="slot"&&S.me.max_devices&&p.sub.devices+next>S.me.max_devices)return;if(p.kind==="traffic"&&p.sub.gb+next*50>100000)return;p.count=next;fixMethod();renderSheet()},
  "pay-go":submitPayment,"pay-check":checkPayment,"pay-open":()=>invoke("open_external",{url:S.pay.link}),
  "resume-payment":()=>{S.sheet="pay";renderSheet();schedulePaymentPoll()},
  "copy-ref":async()=>{await navigator.clipboard.writeText(S.referral.ref_link_web||S.referral.ref_link);toast("Link copied")},
  "dev-del":async el=>{
    if(S.deviceBusy)return;const id=el.dataset.id,subId=S.activeSub;if(S.edit!=="x"+id)return;const d=S.devices[subId].find(d=>d.hwid===id);if(!d)return;
    S.deviceBusy=true;el.disabled=true;for(const control of $$('[data-act^="dev-"],[data-act="switch"],[data-act="switch-ok"]'))control.disabled=true;
    try{if(d.me&&S.conn==="connected"){await invoke("disconnect");setConn("idle")}await api("DELETE",`/devices/${encodeURIComponent(id)}?sub_id=${subId}`);
      if(d.me){await invoke("clear_subscription",{expectedSubId:subId});engineBoot(await invoke("boot"))}
      collapse(el.closest(".dev"),()=>{S.devices[subId]=(S.devices[subId]||[]).filter(device=>device.hwid!==id);if(S.screen==="devices"&&S.activeSub===subId){S.edit=null;rr()}toast("Device removed.")})
    }finally{S.deviceBusy=false;for(const control of $$('[data-act^="dev-"],[data-act="switch"],[data-act="switch-ok"]'))control.disabled=false;if(el.isConnected)el.disabled=false}
  }
});
let resendTimer=0;
act.auth=el=>{if(S.authBusy)return;const input=$('#email');if(input)S.auth.email=input.value.trim();S.auth.mode=el.dataset.v;rr()};
act['auth-resend']=async()=>{if(S.authBusy||Date.now()<S.auth.retryAt)return;S.authBusy=true;try{await invoke('cabinet_auth',{action:'resend',body:{email:S.auth.email}});S.auth.retryAt=Date.now()+60000;toast('A new code has been sent.');clearTimeout(resendTimer);resendTimer=setTimeout(()=>{if(!S.signed_in&&S.auth.mode==='verify')rr()},60000)}catch(error){report(error)}finally{S.authBusy=false;rr()}};
document.addEventListener('submit',async event=>{
 const form=event.target;if(!['login','register','verify','forgot','reset','promo'].includes(form.id))return;event.preventDefault();if(S.authBusy)return;
 const button=form.querySelector('button[type="submit"]')||form.querySelector('button');button.disabled=true;button.classList.add('loading');S.authBusy=true;
 try{
  if(form.id==='promo'){
    const code=$('#promo-code').value.trim();if(!code){toast('Enter a promo code.');$('#promo-code').focus();return}
    await api('POST','/promo',{code});await refreshMe();toast('Promo code applied.');
    if(S.sheet==='promo'&&$('#promo')===form)closeSheet();paintScreen(S.screen);return
  }
  const email=$('#email')?.value.trim()||S.auth.email,password=$('#password')?.value||'';let result;
  if(form.id==='login'){result={me:await invoke('cabinet_login',{email,password})}}
  else{let body={email};if(form.id==='register')body={email,password,accept_terms:$('#terms').checked,accept_personal_data:$('#personal-data').checked};if(form.id==='verify')body={email:S.auth.email,code:$('#code').value.trim()};if(form.id==='reset'){let token=$('#reset-link').value.trim();if(/^https?:\/\//i.test(token)){const link=new URL(token);if(link.origin!==new URL(S.cabinet.cabinet_url).origin)throw Error('invalid reset link');token=link.searchParams.get('token')||''}body={token,password}}result=await invoke('cabinet_auth',{action:form.id,body})}
  S.auth.email=email;
  if(result.me){applyMe(result.me);S.auth.mode='login';engineBoot(await invoke('boot'));go('home');paintScreen('home')}
  else if(form.id==='register'){S.auth.mode='verify';S.auth.email=result.email||email;toast('Check your email for a verification code.');rr()}
  else if(form.id==='forgot'){S.auth.mode='reset';rr()}
 }catch(error){report(error)}finally{S.authBusy=false;if(button.isConnected){button.disabled=false;button.classList.remove('loading')}const current=$('form.auth button[type="submit"]');if(current)current.disabled=false}
});

document.addEventListener("change",event=>{if(event.target.id==="topup-amount"&&S.pay&&!S.pay.busy){S.pay.amount=Math.round(Number(event.target.value)*100);renderSheet()}});
document.addEventListener("visibilitychange",()=>{if(document.hidden){cancelAnimationFrame(frameHandle);frameHandle=0;stopPaymentPoll()}else{startAnimation();if(S.pay?.step==="wait"&&S.sheet==="pay")checkPayment().catch(report);refreshScreen(S.screen).catch(report)}watchTraffic().catch(report)});
async function boot(){
  readTheme();resample();buildShell();render();$("#main").classList.add("enter");entered();
  void loadLegal();
  await window.__TAURI__.event.listen("tick",({payload:p})=>{
    S.secs=p.secs;S.dn=p.dn;S.up=p.up;
    [["#timer",clock(p.secs)],["#bt",clock(p.secs)],["#dn",rate(p.dn)],["#up",rate(p.up)],["#bdn",rate(p.dn)],["#bup",rate(p.up)]].forEach(([selector,value])=>{const el=$(selector);if(el)el.textContent=value});
    if(p.conns&&!S.paused){const counts=new Map();const previous=new Map(S.conns.map(c=>[c.id,c]));S.conns=p.conns.map(c=>{const signature=JSON.stringify([c.host,c.port,c.proc,c.chains]),i=counts.get(signature)||0;counts.set(signature,i+1);const key=c.id||signature+":"+i;let id=trafficIds.get(key);if(!id){id=++uid;trafficIds.set(key,id)}const target=(c.chains||[]).some(n=>/reject|block/i.test(n))?"@block":(c.chains||[]).some(n=>/^direct$/i.test(n))?"@direct":"@proxy";const own=S.rules.find(r=>r.on&&r.type.replace(/[-_]/g,"").toUpperCase()===(c.rule||"").replace(/[-_]/g,"").toUpperCase()&&r.value===c.payload);return{...c,host:c.host||"",proc:c.proc||"",id,route:TARGET[target][1],origin:own?"user":c.rule==="Match"||c.rule==="MATCH"?"default":"plan",rule:c.rule&&c.rule.toUpperCase()!=="MATCH"?{type:c.rule,value:c.payload||"",target}:null,dirty:!previous.has(id)||previous.get(id).route!==TARGET[target][1]||previous.get(id).rule?.value!==(c.payload||"")}});const ids=new Set(S.conns.map(c=>c.id));for(const[id,el]of connEls)if(!ids.has(id)){collapse(el,()=>el.remove());connEls.delete(id)}if(trafficIds.size>1000)for(const[key,id]of trafficIds)if(!ids.has(id))trafficIds.delete(key);if(S.screen==="conns")syncConns()}
  });
  await window.__TAURI__.event.listen("tray-toggle",()=>{toggle().catch(report)});
  await window.__TAURI__.event.listen("dropped",()=>{S.conns=[];connEls.clear();S.secs=0;setConn("idle");if(["home","conns"].includes(S.screen))rr();toast("Disconnected. The VPN core stopped.")});
  const core=await invoke("boot");engineBoot(core);paintScreen(S.screen);
  try{S.cabinet=await invoke("cabinet_boot");S.signed_in=S.cabinet.signed_in;if(S.cabinet.me)applyMe(S.cabinet.me);if(S.cabinet.error)report(S.cabinet.error);paintScreen(S.screen)}catch(error){report(error)}
  if(S.has_sub)await loadNodes();if(core.connect_on_boot&&S.signed_in&&S.me&&S.has_sub&&S.conn!=="connected")await toggle();
  document.fonts?.ready.then(markNav);setTimeout(markNav,300);
}
const trafficIds=new Map();
boot().catch(report);
