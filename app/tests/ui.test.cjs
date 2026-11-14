const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
const prototype = fs.readFileSync(path.join(__dirname, '../../docs/moonlight/prototype/minimal.html'), 'utf8');
const nodes = new Map(), handlers = {}, events = {}, calls = [], storage = {};
const classes = () => {const values=new Set();return {add:(...v)=>v.forEach(x=>values.add(x)),remove:(...v)=>v.forEach(x=>values.delete(x)),contains:v=>values.has(v),toggle:()=>{}}};
function element() {
  return {dataset:{},style:{setProperty(){}},classList:classes(),children:[],innerHTML:'',textContent:'',hidden:false,isConnected:true,offsetWidth:1060,
    setAttribute(){},getAttribute(){return null},removeAttribute(){},focus(){},select(){},append(){},prepend(){},remove(){this.isConnected=false},
    getBoundingClientRect:()=>({left:0,top:0,width:10,height:10}),querySelector:query,querySelectorAll:()=>[],matches:()=>false,contains:()=>false,
    animate(){},getAnimations:()=>[],addEventListener(){},getContext:()=>({scale(){},stroke(){},beginPath(){},arc(){},fill(){},getImageData(){const data=new Uint8Array(256*256*4);data[3]=255;return {data}}})};
}
function query(selector) {if(!nodes.has(selector))nodes.set(selector,element());return nodes.get(selector)}
const me = {id:1,email:'fixture@example.invalid',balance:100000,discount:{percent:7},payment_methods:[{id:'yookassa',name:'Cards',fee_percent:5}],subscriptions:[
  {id:77,plan:'Fixture <plan>',tariff_id:2,status:'active',days_left:30,term_days:90,expires_at:'2027-01-01T00:00:00Z',traffic_gb:500,used_gb:125,devices:5,plan_devices:5,plan_traffic_gb:500,extra_device_price_kopecks:1500,extra_gb_price_kopecks:5000,addon_months_remaining:2,renew_price_per_30_kopecks:25001},
  {id:78,plan:'Second',tariff_id:2,status:'active',days_left:60,term_days:90,expires_at:'2027-02-01T00:00:00Z',traffic_gb:100,used_gb:0,devices:3}
]};
let latencyWait=null,offline=false,telegramAnswers=false,deleteWait=null,buyTerminal=false,buyWait=null,connectWait=null,connectError=null,closeError=null,buyFails=1,promoFails=false,core={has_sub:true,connected:false,rules:[],mode:'tun',node:'Fixture <server>',hwid:'own/hwid',sub_id:77,version:'0.1.0'};
const document={hidden:false,documentElement:element(),activeElement:element(),fonts:{ready:Promise.resolve()},querySelector:query,querySelectorAll:()=>[],createElement:element,addEventListener:(name,fn)=>(handlers[name]??=[]).push(fn)};
const context = vm.createContext({document,window:{__TAURI__:{event:{listen:async(name,fn)=>events[name]=fn},core:{Channel:class {},invoke:async(name,args)=>{
  calls.push([name,args]);
  if(name==='boot')return {...core};
  if(name==='cabinet_boot')return {configured:true,signed_in:true,base_url:'https://example.invalid/api/v1',cabinet_url:'https://example.invalid',me:structuredClone(me)};
  if(name==='cabinet_login'){core.has_sub=false;core.sub_id=null;return structuredClone(me);}
  if(name==='cabinet_telegram_login'){if(!telegramAnswers)return null;core.has_sub=false;core.sub_id=null;return structuredClone(me);}
  if(name==='cabinet_auth'){if(args.action==='register')return {status:'verification_required',email:args.body.email};if(args.action==='verify'||args.action==='reset')return {status:'signed_in',me:structuredClone(me)};return {status:'reset_sent'}}
  if(name==='refresh_subscription')return {...core,profile:{title:'Fixture profile',total:0,upload:12,download:34,updated_at:1,update_hours:1}};
  if(name==='connect'){if(connectWait)return await connectWait;if(connectError)throw connectError;core.connected=true;return {ms:300}}
  if(name==='disconnect'){core.connected=false;return {ok:true}}
  if(name==='close_connections'){if(closeError)throw closeError;return {ok:true}}
  if(name==='clear_subscription'){if(args?.expectedSubId&&args.expectedSubId!==core.sub_id)return {ok:true};core.has_sub=false;core.sub_id=null;return {ok:true}}
  if(name==='cabinet_use_subscription'){if(offline)throw 'Cabinet connection failed. Check your internet and API address.';core.sub_id=args.subId;core.has_sub=true;return {...core}}
  if(name==='test_latency'&&latencyWait)return latencyWait(args);
  if(name==='proxies'||name==='test_latency')return {nodes:[{name:'Fixture <server>',delay:30}],now:'Fixture <server>'};
  if(name==='cabinet_request'){
    if(offline)throw 'Cabinet connection failed. Check your internet and API address.';
    if(args.path==='/legal')return {version:'2026-10-21',docs:[{key:'offer',title:'Public offer',url:'https://example.invalid/offer'},{key:'privacy',title:'Privacy policy',url:'https://example.invalid/privacy'}]};
    if(args.path==='/me/transactions')return {transactions:[{id:1,type:'topup',amount_kopecks:1234,status:'completed',created_at:'2026-11-10T10:00:00Z',description:'Fixture credit'},{id:2,type:'purchase',amount_kopecks:-567,status:'completed',created_at:'2026-11-09T10:00:00Z',description:'<plan>'}]};
    if(args.path==='/promo'&&promoFails)throw 'HTTP 503 internal failure';
    if(args.path==='/me')return structuredClone(me);
    if(args.path==='/tariffs')return {tariffs:[{id:2,name:'Fixture <tariff>',price_kopecks:25001,devices:5,traffic_gb:500}]};
    if(args.path.startsWith('/devices?'))return {devices:[{hwid:'own/hwid',name:'Test <device>',platform:'Windows',os:'11'}],max:5};
    if(args.path.startsWith('/devices/')){if(args.method==='DELETE'&&deleteWait)return await deleteWait;return {ok:true}};
    if(args.path==='/buy'){if(buyTerminal)return {ok:true,status:'compensated'};if(buyWait)return await buyWait;if(buyFails-->0)throw 'Fixture timeout';return {ok:true,link:'https://payments.example.invalid/invoice',status:'pending'}}
    if(args.path.startsWith('/checkout/'))return {ok:true,status:'pending'};
    if(args.path.includes('/extra-gb'))return {ok:true,link:'https://payments.example.invalid/traffic'};
    return {ok:true};
  }
  return {ok:true};
}}}},performance:{now:()=>0},Path2D:function(){},getComputedStyle:()=>({getPropertyValue:()=>'#fff'}),
  matchMedia:()=>({matches:true}),requestAnimationFrame:()=>1,cancelAnimationFrame:()=>{},setTimeout:()=>1,clearTimeout:()=>{},setInterval:()=>1,clearInterval:()=>{},
  addEventListener(){},structuredClone,console,URL,crypto:require('node:crypto').webcrypto,
  localStorage:{getItem:key=>storage[key]??null,setItem:(key,value)=>storage[key]=value,removeItem:key=>delete storage[key]},navigator:{clipboard:{writeText:async()=>{}}}
});
const run = code => vm.runInContext(code,context);
vm.runInContext(source,context);
const flush = () => new Promise(resolve=>setImmediate(resolve));
(async()=>{
  await flush();
  run('engineBoot({...S,has_sub:false,sub_id:77,connected:false});applyMe('+JSON.stringify(me)+')');
  const missingProfileStart=calls.length;
  let releaseProfileConnect;connectWait=new Promise(resolve=>releaseProfileConnect=resolve);
  const missingProfileConnect=run('toggle()');await flush();
  assert(calls.slice(missingProfileStart).some(([name])=>name==='cabinet_use_subscription'),'a saved plan id without a cached profile must reload its subscription before connecting');
  assert.equal(run('S.conn'),'connecting','loading a plan must preserve the pending connection state');
  await run('toggle()');assert.equal(calls.at(-1)[0],'disconnect','a first connection remains cancellable after loading its plan');
  releaseProfileConnect({ms:10});await missingProfileConnect;connectWait=null;assert.equal(run('S.conn'),'idle');
  assert.equal(run('friendlyError("Internal error (HTTP 503) database failure")'),'Something went wrong. Try again later.');
  assert.equal(run('friendlyError("Wrong email or password. (HTTP 401)")'),'Wrong email or password.');
  assert.equal(run('S.logo'),3);
  assert.equal(run('N'),900);
  run('S.conn="idle";S.has_sub=true');await run('act.nodes()');
  assert.equal(run('S.sheet'),'nodes','server picker opens before connecting');
  assert.match(run('nodesBody().foot'),/choice will be used when you connect/);
  await run('act.node({dataset:{v:"Fixture <server>"}})');
  assert(calls.some(([name,args])=>name==='select_node'&&args.name==='Fixture <server>'));
  await run('act.test()');assert.equal(run('S.conn'),'idle');assert.match(run('nodesBody().foot'),/Update subscription/);
  run('S.profile={announcement:"Fixture <announcement>"};renderSide()');assert.match(query('#side').innerHTML,/Fixture &lt;announcement&gt;/);assert(query('#side').innerHTML.indexOf('class="card pad col announcement"')<query('#side').innerHTML.indexOf('data-act="nodes"'),'announcement is above the server selector');assert.doesNotMatch(run('screens.settings()'),/Fixture/);
  run('S.sheet=null');
  assert.match(run('nodeFlag("🇫🇮 Finland LTE auto")'),/flags\/fi\.svg/);
  assert.equal(run('nodeLabel("🇫🇮 Finland LTE auto")'),'Finland LTE auto');
  assert.equal(run('nodeLabel("EE Estonia LTE")'),'Estonia LTE');
  run('S.subs[0].devices=0;S.subs[0].gb=0;S.deviceLoaded[77]=true;S.activeSub=77');
  assert.doesNotMatch(run('screens.devices()'),/data-act="pay-slot"/);
  assert.doesNotMatch(run('screens.subs()'),/data-act="pay-traffic" data-id="77"/);
  run('S.subs[0].tariff=null');
  assert.doesNotMatch(run('screens.subs()'),/data-act="pay-renew" data-id="77"/,'active custom plan must not ask to choose a plan');
  run('S.subs[0].tariff=2;S.subs[0].devices=5;S.subs[0].gb=500');
  const processGroups=run('groupConnections([{proc:"C:\\\\Apps\\\\chrome.exe",host:"one",dn:2,up:1},{proc:"chrome.exe",host:"two",dn:3,up:2},{proc:"Discord.exe",host:"three",dn:1,up:0}])');
  assert.equal(processGroups.length,2);assert.equal(processGroups[0].connections.length,2);assert.equal(processGroups[0].dn,5);
  run('S.screen="account";document.querySelector("#main").classList.add("enter");document.querySelector("#main").innerHTML="entry-in-progress";paintScreen("account")');
  assert.equal(query('#main').innerHTML,'entry-in-progress','background refresh must not restart the screen entry');
  run('document.querySelector("#main").classList.remove("enter")');
  const animationBody = (text,name,next) => text.slice(text.indexOf(`function ${name}(`),text.indexOf(next,text.indexOf(`function ${name}(`))).trim();
  assert.equal(animationBody(source,'dust','let amp='),animationBody(prototype,'dust','let amp='),'dust particle choreography must remain the original');
  assert.equal(animationBody(source,'resample','function dust('),animationBody(prototype,'resample','function dust('),'logo resampling must remain the original');
  assert.equal(run(`esc('<img src=x onerror="alert(1)">')`),'&lt;img src=x onerror=&quot;alert(1)&quot;&gt;');
  assert.equal(run('clock(3661)'),'01:01:01');
  assert.match(run('screens.account()'),/fixture@example.invalid/);
  assert.doesNotMatch(run('screens.account()'),/anna@example/);
  assert.match(run('screens.account()'),/class="grp profile"[\s\S]*data-act="logout"[\s\S]*Time left[\s\S]*Traffic left[\s\S]*Plan<[\s\S]*Connection<[\s\S]*Bonuses<[\s\S]*data-v="referral"[\s\S]*data-v="promo"[\s\S]*Application</,'profile card comes first with Sign out inside; Bonuses holds referral and promo');
  assert.equal(run('typeof screens.balance'),'function','Balance has its own subpage');
  assert.doesNotMatch(run('screens.subs()'),/data-act="pay-topup"|Balance/,'Plans does not contain the wallet');
  assert.match(run('screens.balance()'),/Balance[\s\S]*data-act="pay-topup"/);
  assert.match(run('screens.account()'),/data-v="balance"/);
  assert.equal(run('screens.account()').match(/data-act="logout"/g).length,1,'one Sign out, in the profile card');
  assert.match(run('screens.settings()'),/Connection<[\s\S]*About<[\s\S]*Legal</,'Settings is grouped');
  assert.match(run('screens.settings()'),/App update[\s\S]*data-act="update-check"/,'About holds the update control');
  run('S.update={state:"available",version:"9.9.9"}');assert.match(run('screens.settings()'),/Version 9\.9\.9 is available[\s\S]*data-act="update-install"/);run('S.update={state:"idle",version:""}');
  // an account that has not loaded is not an empty account, and a failed background read is silent
  run('var keepMe=S.me,keepSubs=S.subs,keepHas=S.has_sub,toasts=0,realToast=toast;S.me=null;S.subs=[];toast=()=>{toasts++}');
  assert.match(run('screens.account()'),/aria-busy="true"/,'unloaded account shows a skeleton');
  assert.doesNotMatch(run('screens.account()'),/temporarily unavailable/);
  assert.doesNotMatch(run('screens.subs()'),/No plans yet/,'a failed load is not an empty account');
  run('S.has_sub=true');assert.equal(await run('ensureSubscription()'),true,'a saved profile stays usable while the account loads');
  run('S.has_sub=false');assert.equal(await run('ensureSubscription()'),false);
  run('background(Error("Cabinet connection failed. Check your internet and API address."))');assert.equal(run('toasts'),0,'background failures do not toast');
  assert.equal(run('retryDelay'),8000,'retry backs off');run('clearTimeout(retryTimer);retryTimer=0;retryDelay=4000');
  assert.match(run('screens.account()'),/data-act="logout"[\s\S]*data-v="rules"[\s\S]*data-v="settings"/,'Account stays usable before the account loads');
  // the last good answers are saved, and serve the screens while the service cannot be reached
  assert.equal(JSON.parse(storage['cab:/me']).id,1,'the account is saved after a successful load');
  offline=true;
  assert.equal((await run('api("GET","/me")')).id,1,'an unreachable service serves the saved account');
  await run('refreshMe()');assert.equal(run('S.me.id'),1);assert.equal(run('toasts'),0);assert.match(run('screens.account()'),/fixture@example\.invalid/);
  await run('loadLegal(true)');assert.match(run('legalRows()'),/Public offer/,'saved documents show offline');
  await assert.rejects(run('api("POST","/promo",{code:"X"})'),/connection failed/,'changes are never answered from saved data');
  await assert.rejects(run('api("GET","/checkout/1")'),/connection failed/,'payment status is never answered from saved data');
  run('S.engine_sub_id=null;S.has_sub=true');assert.equal(await run('ensureSubscription()'),true,'a saved profile connects when the plan cannot be fetched');
  offline=false;run('clearTimeout(retryTimer);retryTimer=0;retryDelay=4000;S.engine_sub_id=77');
  run('S.me=keepMe;S.subs=keepSubs;S.has_sub=keepHas;toast=realToast');
  assert.match(run('screens.subs()'),/data-flip="subs"/);
  assert.doesNotMatch(run('screens.settings()'),/Appearance|Advanced|Account service|<h2 class="over">Subscription|data-act="theme"/);
  assert.match(run('screens.settings()'),/identification is required and cannot be disabled/);
  await run('act.legal()');assert.equal(run('S.sheet'),'legal');assert.match(run('legalRows()'),/Public offer[\s\S]*Privacy policy/);await run('act["legal-doc"]({dataset:{i:1}})');assert.equal(calls.at(-1)[1].url,'https://example.invalid/privacy');run('closeSheet()');
  await run('loadHistory()');assert.match(run('historyCard()'),/\+12\.34 ₽/);assert.match(run('historyCard()'),/−5\.67 ₽/);assert.match(run('historyCard()'),/&lt;plan&gt;/);assert.doesNotMatch(run('historyCard()'),/<plan>/);
  assert.match(run('screens.settings()'),/aria-checked="true" data-act="close-tray"/,'close to tray is on by default');
  await run('act["close-tray"]()');assert.equal(JSON.stringify(calls.at(-1)),'["set_close_to_tray",{"on":false}]');assert.match(run('screens.settings()'),/aria-checked="false" data-act="close-tray"/);
  await run('act["close-tray"]()');assert.equal(run('S.closeToTray'),true);
  run('S.screen="account";act.promo()');assert.equal(run('S.screen'),'account');assert.equal(run('S.sheet'),'promo');assert.match(run('promoBody().body'),/id="promo"[\s\S]*id="promo-code"/);
  const promoForm={id:'promo',querySelector:()=>element()};nodes.set('#promo',promoForm);
  promoFails=true;query('#promo-code').value='FIXTURE-CODE';await handlers.submit[0]({target:promoForm,preventDefault(){}});assert.equal(run('S.sheet'),'promo');assert.equal(query('#promo-code').value,'FIXTURE-CODE');assert.equal(run('S.authBusy'),false);
  promoFails=false;await handlers.submit[0]({target:promoForm,preventDefault(){}});
  assert(calls.some(([name,args])=>name==='cabinet_request'&&args.path==='/promo'&&args.body.code==='FIXTURE-CODE'));assert.equal(run('S.sheet'),null);
  run(`go('account');go('settings')`);
  assert.equal(run('S.screen'),'settings');
  assert.match(run(`head('Settings')`),/data-v="account"/,'Settings returns to Account');
  for(const screen of ['conns','rules','subs','devices','referral']) {run(`S.screen='${screen}'`);assert.match(run(`head('Child')`),/data-v="account"/)}
  run(`go('account')`);assert.match(run(`head('Account')`),/data-v="home"/,'root account returns Home');
  run(`go('home');go('subs')`);assert.match(run(`head('Plans')`),/data-v="home"/,'Plans opened from Home returns Home');
  run(`go('account');go('subs')`);assert.match(run(`head('Plans')`),/data-v="account"/,'Plans opened from Account returns Account');
  run(`go('account')`);
  let releaseConnect;connectWait=new Promise(resolve=>releaseConnect=resolve);
  const connecting=run('toggle()');await flush();assert.equal(run('S.conn'),'connecting');
  await run('toggle()');assert.equal(calls.at(-1)[0],'disconnect');releaseConnect({ms:300});await connecting;connectWait=null;
  assert.equal(run('S.conn'),'idle','late completion cannot reconnect a canceled attempt');assert.equal(run('S.busy'),false);
  await run('toggle()');
  assert.equal(run('S.conn'),'connected');
  run('renderStatus()');assert.doesNotMatch(query('#status').innerHTML,/From your subscription|Fixture &lt;server&gt;/,'connected status shows the timer and traffic without the server subtitle');
  assert.match(run('msHTML({id:"fixture",status:"failed",ms:null})'),/Test failed/);
  run('applyNodes({now:"Fixture <server>",nodes:[{name:"Fixture <server>",delay:30},{name:"Slow",status:"unknown"}]})');
  let progress,releaseLatency;
  latencyWait=args=>{progress=args?.onResult;return new Promise(resolve=>releaseLatency=resolve)};
  const testing=run('act.test()');await flush();
  assert.equal(run('S.testing'),true);
  assert(progress,'latency command must accept a progress channel');
  progress.onmessage({name:'Fixture <server>',delay:18,status:'ok',url:'https://example.invalid/check'});
  assert.equal(run('NODES[0].ms'),18,'a finished server updates while another server is pending');
  assert.equal(run('NODES[1].ms'),null);assert.equal(run('S.node'),'Fixture <server>');
  progress.onmessage({name:'Removed server',delay:1,status:'ok'});
  assert.equal(run('NODES.length'),2,'late results cannot restore removed servers');
  releaseLatency({now:'Fixture <server>',nodes:[{name:'Fixture <server>',delay:18,status:'ok'},{name:'Slow',status:'failed'}]});
  await testing;latencyWait=null;assert.equal(run('S.testing'),false);assert.equal(run('NODES[1].status'),'failed');
  run('S.testing=true');await run('toggle()');assert.equal(run('S.conn'),'idle','latency testing must not block Disconnect');run('S.testing=false');await run('toggle()');
  await run(`go('conns');watchTraffic()`);
  assert(calls.some(([name,args])=>name==='watch'&&args.on));
  await assert.rejects(run(`act.mode({dataset:{v:'proxy'}})`),/Disconnect/);
  await run('toggle()');assert.equal(run('S.conn'),'idle');
  connectError='elevation_cancelled';await assert.rejects(run('toggle()'),/elevation_cancelled/);assert.equal(run('S.conn'),'idle');assert(!calls.some(([name])=>name==='elevate'),'TUN must never restart the app');connectError=null;
  await run(`act.mode({dataset:{v:'proxy'}})`);assert.equal(calls.at(-1)[0],'set_mode');
  await run('loadDevices(77)');assert.match(run('screens.devices()'),/Test &lt;device&gt;/);assert.match(run('screens.devices()'),/This device/);
  run(`S.edit='down/hwid'`);await run(`saveDevName({dataset:{id:'own/hwid'},value:'New name'})`);
  assert(calls.some(([name,args])=>name==='cabinet_request'&&args.method==='PATCH'&&args.path==='/devices/own%2Fhwid?sub_id=77'&&args.body.name==='New name'));
  await run(`openPay('buy')`);
  assert.equal(run('S.pay.step'),'plan');
  const before=calls.filter(([name,args])=>name==='cabinet_request'&&args.method==='POST').length;
  run(`act['pay-step']({dataset:{v:'method'}})`);
  assert.equal(calls.filter(([name,args])=>name==='cabinet_request'&&args.method==='POST').length,before,'choosing a method must not create an invoice');
  run(`S.pay.method='yookassa'`);
  await run('submitPayment()');assert.equal(run('S.pay.step'),'fail');
  const first=calls.find(([name,args])=>name==='cabinet_request'&&args.path==='/buy')[1].body.request_id;
  await run('submitPayment()');assert.equal(run('S.pay.step'),'wait');
  const buyCalls=calls.filter(([name,args])=>name==='cabinet_request'&&args.path==='/buy');
  assert.equal(buyCalls[1][1].body.request_id,first,'uncertain retry must reuse its UUID');
  await run('checkPayment()');assert.equal(run('S.pay.step'),'wait','checking pending payments cannot simulate success');
  run('closeSheet()');assert.equal(run('S.pay.step'),'wait','pending payment remains accessible after closing');
  run('S.pay=null;S.sheet=null');await run("openPay('buy')");run("S.pay.days=90;S.pay.method='yookassa'");
  let releaseBuy;buyWait=new Promise(resolve=>releaseBuy=resolve);const submitting=run('submitPayment()');await flush();
  run('act["pay-method"]({dataset:{v:"balance"}});act["pay-days"]({dataset:{v:30}})');
  assert.equal(run('S.pay.method'),'yookassa','payment method is frozen during submission');assert.equal(run('S.pay.days'),90,'period is frozen during submission');
  run('S.pay.method="balance"');releaseBuy({ok:true,link:'https://payments.example.invalid/invoice'});await submitting;buyWait=null;
  assert.equal(run('S.pay.step'),'wait','provider response is classified by the immutable submitted method');
  assert(storage[Object.keys(storage).find(key=>storage[key].includes(run('S.pay.request.id')))],'pending external invoice retains its idempotency record');
  run(`S.pay=null;S.sheet=null`);await run(`openPay('renew',77)`);run(`S.pay.days=90;S.pay.method='yookassa'`);
  const list=Math.trunc(25001*2.7),net=list-Math.floor(list*.07);assert.equal(run('quote().total'),Math.trunc(net*1.05));
  run(`S.pay=null;S.sheet=null`);await run(`openPay('traffic',77)`);
  assert.equal(run('quote().net'),10000,'add-ons use authoritative remaining billing months');
  run(`S.pay.method='yookassa'`);await run('submitPayment()');
  assert(calls.some(([name,args])=>name==='cabinet_request'&&args.path==='/subscription/77/extra-gb'&&args.body.new_total_gb===550));
  me.subscriptions[0].traffic_gb=550;await run('checkPayment()');assert.equal(run('S.pay.step'),'wait','unrelated entitlement changes do not prove an invoice paid');me.subscriptions[0].traffic_gb=500;
  run('S.pay=null;S.sheet=null');await run("openPay('buy')");run('S.pay.days=180');buyTerminal=true;await run('submitPayment()');
  const terminalID=calls.findLast(([name,args])=>name==='cabinet_request'&&args.path==='/buy')[1].body.request_id;
  assert.equal(run('S.pay.request'),null,'verified compensated checkout retires its request');
  buyTerminal=false;await run('submitPayment()');assert.notEqual(calls.findLast(([name,args])=>name==='cabinet_request'&&args.path==='/buy')[1].body.request_id,terminalID,'explicit retry after compensation creates a new UUID');
  run(`S.pay=null;S.sheet=null`);await run(`useSubscription(78)`);assert.equal(run('S.activeSub'),78);assert(calls.some(([name,args])=>name==='cabinet_use_subscription'&&args.subId===78));
  run(`S.screen='home';S.has_sub=true;S.engine_sub_id=S.activeSub`);await run('toggle()');
  await run(`useSubscription(77)`);
  const switchTail=calls.slice(calls.findLastIndex(([name])=>name==='cabinet_use_subscription')-1).map(([name])=>name);
  assert.equal(switchTail[0],'disconnect');assert(switchTail.includes('connect'),'switch confirmation reconnects a connected user');
  await run(`loadDevices(77);S.edit='xown/hwid'`);
  const marker=calls.length;let releaseDelete;deleteWait=new Promise(resolve=>releaseDelete=resolve);
  const revoking=run(`act['dev-del']({dataset:{id:'own/hwid'},disabled:false,closest:()=>null})`);await flush();
  core.sub_id=78;core.connected=true;run('S.activeSub=78;S.devices[78]=[{hwid:"keep"}];S.edit="dkeep"');releaseDelete({ok:true});await revoking;deleteWait=null;
  assert.equal(run('S.activeSub'),78);assert.equal(run('S.devices[78][0].hwid'),'keep','late revoke callback cannot mutate another plan');assert.equal(run('S.edit'),'dkeep');assert.equal(core.has_sub,true,'conditional clear cannot remove another subscription');
  const revoke=calls.slice(marker).map(([name])=>name);assert.equal(revoke[0],'disconnect');assert(revoke.includes('clear_subscription'));
  run(`S.conn='connected';S.conns=[]`);await events.tick({payload:{secs:5,dn:2048,up:1024,conns:[{id:'real-socket',host:'example.invalid',port:443,proc:'fixture.exe',dn:2048,up:1024,chains:['DIRECT'],rule:'DomainSuffix',payload:'example.invalid'}]}});
  assert.equal(run('S.conns[0].route'),'direct');assert.equal(run('S.secs'),5);
  assert.equal(run('S.conns[0].connectionId'),'real-socket','closing needs the core ID, not the numeric row ID');
  const closeButton={dataset:{id:run('S.conns[0].id')},disabled:false};
  closeError='Fixture close failure';await assert.rejects(run(`act['conn-close'](${JSON.stringify(closeButton)})`),/Fixture close failure/);closeError=null;
  assert.equal(run('S.conns.length'),1,'a failed close keeps the connection visible');
  await run(`act['conn-close'](${JSON.stringify(closeButton)})`);
  assert.equal(JSON.stringify(calls.at(-1)),JSON.stringify(['close_connections',{id:'real-socket',process:null}]));assert.equal(run('S.conns.length'),0);
  await events.tick({payload:{secs:6,dn:0,up:0,conns:[{id:'one',host:'one',proc:'Fixture.exe',chains:[]},{id:'two',host:'two',proc:'C:\\Apps\\fixture.exe',chains:[]},{id:'other',host:'other',proc:'other.exe',chains:[]}]}});
  run('S.paused=true');await run(`act['conn-close']({dataset:{process:'fixture.exe'},disabled:false})`);
  assert.equal(JSON.stringify(calls.at(-1)),JSON.stringify(['close_connections',{id:null,process:'fixture.exe'}]));assert.equal(run('S.conns.length'),1);assert.equal(run('S.conns[0].connectionId'),'other','closing a process leaves other processes alone');run('S.paused=false');

  await events.dropped();assert.equal(run('S.conn'),'idle');
  run('S.signed_in=false;S.screen="account"');assert.match(run('screens.account()'),/id="email"/);assert.doesNotMatch(run('screens.account()'),/API URL/);
  query('#email').value='fixture@example.invalid';query('#password').value='fixture-only-password';
  await handlers.submit[0]({target:{id:'login',querySelector:()=>element()},preventDefault(){}});
  assert(calls.some(([name,args])=>name==='cabinet_login'&&args.email==='fixture@example.invalid'));
  assert.equal(run('S.signed_in'),true);assert(!Object.values(storage).some(value=>value.includes('fixture-only-password')),'credentials must never enter frontend storage');
  run('NODES.push({id:"<node>",name:"<node>",sub:"<profile>",cc:"xx",ms:30})');
  assert.doesNotMatch(run('nodesBody().body'),/<node>/,'subscription node names are escaped');
  run('S.sheet="nodes"');handlers.keydown[0]({key:'Tab',target:element(),shiftKey:false,preventDefault(){}});
  // Telegram sign-in: offered beside the email form; a closed window changes nothing, an answer signs in
  run('S.signed_in=false;S.me=null;S.auth.mode="login";S.screen="account"');assert.match(run('signInScreen()'),/data-act="auth-telegram"[^>]*>Continue with Telegram/);
  run('S.auth.mode="forgot"');assert.doesNotMatch(run('signInScreen()'),/auth-telegram/);run('S.auth.mode="login"');
  await run('act["auth-telegram"]()');assert.equal(run('S.signed_in'),false,'closing the Telegram window does not sign in');assert.equal(run('S.authBusy'),false);
  telegramAnswers=true;await run('act["auth-telegram"]()');assert.equal(run('S.signed_in'),true);assert.equal(run('S.me.id'),1);assert.equal(run('S.screen'),'home');telegramAnswers=false;
  run('S.signed_in=false;S.auth.mode="register"');assert.match(run('signInScreen()'),/id="terms"/);assert.match(run('signInScreen()'),/id="personal-data"/);
  query('#terms').checked=true;query('#personal-data').checked=true;
  await handlers.submit[0]({target:{id:'register',querySelector:()=>element()},preventDefault(){}});
  assert.equal(run('S.auth.mode'),'verify');assert(calls.some(([name,args])=>name==='cabinet_auth'&&args.action==='register'&&args.body.accept_terms&&args.body.accept_personal_data));
  query('#code').value='123456';await handlers.submit[0]({target:{id:'verify',querySelector:()=>element()},preventDefault(){}});assert.equal(run('S.signed_in'),true);
  run('S.signed_in=false;S.auth.mode="forgot"');await handlers.submit[0]({target:{id:'forgot',querySelector:()=>element()},preventDefault(){}});assert.equal(run('S.auth.mode'),'reset');
  query('#reset-link').value='https://example.invalid/reset?token=fixture-reset-token';query('#password').value='another-fixture-password';
  await handlers.submit[0]({target:{id:'reset',querySelector:()=>element()},preventDefault(){}});
  assert(calls.some(([name,args])=>name==='cabinet_auth'&&args.action==='reset'&&args.body.token==='fixture-reset-token'));assert.equal(run('S.signed_in'),true);
  run('S.signed_in=false;S.activeSub=null');query('#email').value='fixture@example.invalid';query('#password').value='fixture-password';
  await handlers.submit[0]({target:{id:'login',querySelector:()=>element()},preventDefault(){}});
  assert.equal(run('S.signed_in'),true);assert.equal(run('S.has_sub'),true,'signing into an existing active plan loads its profile automatically');assert.equal(run('S.engine_sub_id'),run('S.activeSub'));assert(run('NODES.length')>0);
  run('S.has_sub=true;S.screen="settings"');await run('act["subscription-refresh"]()');assert.equal(run('S.profile.title'),'Fixture profile');assert.equal(run('S.profile.total'),0);
  const html=fs.readFileSync(path.join(__dirname,'../ui/index.html'),'utf8');assert.doesNotMatch(html,/<script>(?!\s*$)|fonts\.googleapis|id="brand"|class="proto"/);
  assert.doesNotMatch(source,/setInterval\(tick|pay-sim|function settlePay|anna@example|function seed\(|cabinet_configure|id="service"/);
  console.log('UI checks passed: original dust/logo animations, back navigation, real engine/UAC, subscription switching, device commands, checkout pricing/idempotency/pending state, native traffic, no demo data.');
})().catch(error=>{console.error(error);process.exitCode=1});
