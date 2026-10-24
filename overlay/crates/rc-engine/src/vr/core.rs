//! Renderer-independent PoC math. Compile tests without Bevy or game assets:
//! rustc --edition=2021 --test core.rs -o core-tests && ./core-tests
//! All positions and distances in this module are metres; +Y is up, -Z is forward.

use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3(pub f32, pub f32, pub f32);
impl V3 {
    pub const ZERO: Self = Self(0.0, 0.0, 0.0);
    pub const UP: Self = Self(0.0, 1.0, 0.0);
    pub fn dot(self, b: Self) -> f32 { self.0 * b.0 + self.1 * b.1 + self.2 * b.2 }
    pub fn cross(self, b: Self) -> Self {
        Self(self.1*b.2-self.2*b.1, self.2*b.0-self.0*b.2, self.0*b.1-self.1*b.0)
    }
    pub fn length(self) -> f32 { self.dot(self).sqrt() }
    pub fn normalized(self) -> Option<Self> {
        let n = self.length();
        (n.is_finite() && n > 1e-7).then(|| self / n)
    }
    pub fn finite(self) -> bool { self.0.is_finite() && self.1.is_finite() && self.2.is_finite() }
    pub fn array(self) -> [f32; 3] { [self.0, self.1, self.2] }
    pub fn at(self, i: usize) -> f32 { self.array()[i] }
    pub fn min(self, b: Self) -> Self { Self(self.0.min(b.0), self.1.min(b.1), self.2.min(b.2)) }
    pub fn max(self, b: Self) -> Self { Self(self.0.max(b.0), self.1.max(b.1), self.2.max(b.2)) }
    pub fn flat(self) -> Self { Self(self.0, 0.0, self.2) }
}
impl Add for V3 { type Output=Self; fn add(self,b:Self)->Self { Self(self.0+b.0,self.1+b.1,self.2+b.2) } }
impl Sub for V3 { type Output=Self; fn sub(self,b:Self)->Self { Self(self.0-b.0,self.1-b.1,self.2-b.2) } }
impl Mul<f32> for V3 { type Output=Self; fn mul(self,b:f32)->Self { Self(self.0*b,self.1*b,self.2*b) } }
impl Div<f32> for V3 { type Output=Self; fn div(self,b:f32)->Self { self*(1.0/b) } }
impl Neg for V3 { type Output=Self; fn neg(self)->Self { self*(-1.0) } }

/// Game positions already decoded by world_position() are NOT raw x1024 integers.
pub fn game_to_metres(p: [f32; 3], units_per_metre: f32) -> Result<V3, &'static str> {
    if !units_per_metre.is_finite() || !(0.1..=10.0).contains(&units_per_metre) {
        return Err("units per metre must be finite and within 0.1..=10");
    }
    let p = V3(p[0], p[2], -p[1]) / units_per_metre;
    if !p.finite() { return Err("non-finite game position"); }
    Ok(p)
}

pub fn yaw_rotate(v: V3, yaw: f32) -> V3 {
    let (s,c) = yaw.sin_cos();
    V3(c*v.0+s*v.2, v.1, -s*v.0+c*v.2)
}

#[derive(Clone, Copy, Debug)]
pub struct Rig { pub origin: V3, pub yaw: f32 }
impl Rig {
    pub fn world(self, local: V3) -> V3 { self.origin + yaw_rotate(local, self.yaw) }
    /// Rotate about the tracked head, NOT the centre of the room.
    pub fn snap(&mut self, radians: f32, tracked_head: V3) {
        let before = self.world(tracked_head);
        self.yaw = (self.yaw + radians + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        self.origin = before - yaw_rotate(tracked_head, self.yaw);
    }
    /// Keep STAGE floor height, head translation and eye separation in real metres.
    pub fn teleport(&mut self, floor_point: V3, tracked_head: V3) {
        self.origin = floor_point - yaw_rotate(tracked_head.flat(), self.yaw);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge { None, Pressed, Released }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateState { Blocked, Ready, Held }
/// Hysteresis and a release-to-arm requirement. Focus/tracking loss must pass None.
#[derive(Debug)]
pub struct TriggerGate { state: GateState }
impl Default for TriggerGate { fn default() -> Self { Self { state: GateState::Blocked } } }
impl TriggerGate {
    pub fn update(&mut self, value: Option<f32>) -> Edge {
        let Some(v) = value.filter(|v| v.is_finite()) else {
            self.state = GateState::Blocked;
            return Edge::None;
        };
        if v <= 0.25 {
            let edge = if self.state == GateState::Held { Edge::Released } else { Edge::None };
            self.state = GateState::Ready;
            edge
        } else if v >= 0.75 && self.state == GateState::Ready {
            self.state = GateState::Held;
            Edge::Pressed
        } else { Edge::None }
    }
    pub fn held(&self) -> bool { self.state == GateState::Held }
    pub fn cancel(&mut self) { self.state = GateState::Blocked; }
}

#[derive(Default, Debug)]
pub struct SnapGate { armed: bool }
impl SnapGate {
    /// Positive stick X means turn right: negative right-handed Y rotation.
    pub fn update(&mut self, stick: Option<f32>) -> f32 {
        let Some(x) = stick.filter(|x| x.is_finite()) else { self.armed=false; return 0.0; };
        if x.abs() <= 0.25 { self.armed=true; }
        if self.armed && x.abs() >= 0.75 {
            self.armed=false;
            -x.signum()*30.0_f32.to_radians()
        } else { 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle { pub a: V3, pub b: V3, pub c: V3 }
impl Triangle {
    pub fn cross(self) -> V3 { (self.b-self.a).cross(self.c-self.a) }
    pub fn centroid(self) -> V3 { (self.a+self.b+self.c)/3.0 }
    pub fn valid(self) -> bool {
        self.a.finite() && self.b.finite() && self.c.finite() && self.cross().length() > 1e-7
    }
    /// Closest point on a non-degenerate triangle (Voronoi region tests).
    pub fn closest(self, p: V3) -> V3 {
        let ab=self.b-self.a; let ac=self.c-self.a; let ap=p-self.a;
        let d1=ab.dot(ap); let d2=ac.dot(ap);
        if d1<=0.0 && d2<=0.0 { return self.a; }
        let bp=p-self.b; let d3=ab.dot(bp); let d4=ac.dot(bp);
        if d3>=0.0 && d4<=d3 { return self.b; }
        let vc=d1*d4-d3*d2;
        if vc<=0.0 && d1>=0.0 && d3<=0.0 { return self.a+ab*(d1/(d1-d3)); }
        let cp=p-self.c; let d5=ab.dot(cp); let d6=ac.dot(cp);
        if d6>=0.0 && d5<=d6 { return self.c; }
        let vb=d5*d2-d1*d6;
        if vb<=0.0 && d2>=0.0 && d6<=0.0 { return self.a+ac*(d2/(d2-d6)); }
        let va=d3*d6-d5*d4;
        if va<=0.0 && (d4-d3)>=0.0 && (d5-d6)>=0.0 {
            return self.b+(self.c-self.b)*((d4-d3)/((d4-d3)+(d5-d6)));
        }
        let denom=1.0/(va+vb+vc);
        self.a+ab*(vb*denom)+ac*(vc*denom)
    }
}

#[derive(Clone, Copy, Debug)]
struct Aabb { lo: V3, hi: V3 }
impl Aabb {
    fn triangle(t: Triangle) -> Self { Self { lo:t.a.min(t.b).min(t.c), hi:t.a.max(t.b).max(t.c) } }
    fn union(self,b:Self)->Self { Self { lo:self.lo.min(b.lo),hi:self.hi.max(b.hi) } }
    fn ray(self,origin:V3,dir:V3,max:f32)->bool {
        let mut lo=0.0_f32; let mut hi=max;
        for axis in 0..3 {
            let o=origin.at(axis); let d=dir.at(axis);
            if d.abs()<1e-9 {
                if o<self.lo.at(axis)-1e-5 || o>self.hi.at(axis)+1e-5 { return false; }
            } else {
                let a=(self.lo.at(axis)-o)/d; let b=(self.hi.at(axis)-o)/d;
                lo=lo.max(a.min(b)); hi=hi.min(a.max(b));
                if hi<lo { return false; }
            }
        }
        true
    }
    fn sphere(self,p:V3,r:f32)->bool {
        let q=V3(p.0.clamp(self.lo.0,self.hi.0),p.1.clamp(self.lo.1,self.hi.1),p.2.clamp(self.lo.2,self.hi.2));
        (q-p).dot(q-p)<=r*r
    }
}
#[derive(Debug)]
struct Node { bounds:Aabb, start:usize, end:usize, children:Option<(usize,usize)> }
#[derive(Debug, Default)]
pub struct Bvh { triangles:Vec<Triangle>, nodes:Vec<Node> }
#[derive(Clone, Copy, Debug)]
pub struct Hit { pub distance:f32, pub point:V3, pub normal:V3 }
impl Bvh {
    pub fn new(triangles:Vec<Triangle>)->Self {
        let mut s=Self { triangles:triangles.into_iter().filter(|t|t.valid()).collect(),nodes:Vec::new() };
        if !s.triangles.is_empty() { s.build(0,s.triangles.len()); }
        s
    }
    pub fn triangles(&self)->&[Triangle] { &self.triangles }
    fn build(&mut self,start:usize,end:usize)->usize {
        let bounds=self.triangles[start+1..end].iter().fold(Aabb::triangle(self.triangles[start]),|a,t|a.union(Aabb::triangle(*t)));
        let id=self.nodes.len();
        self.nodes.push(Node { bounds,start,end,children:None });
        if end-start>8 {
            let extent=bounds.hi-bounds.lo;
            let axis=if extent.0>=extent.1 && extent.0>=extent.2 {0} else if extent.1>=extent.2 {1} else {2};
            let mid=(start+end)/2;
            self.triangles[start..end].select_nth_unstable_by(mid-start,|a,b|a.centroid().at(axis).total_cmp(&b.centroid().at(axis)));
            let left=self.build(start,mid); let right=self.build(mid,end);
            self.nodes[id].children=Some((left,right));
        }
        id
    }
    pub fn ray(&self,origin:V3,direction:V3,max:f32)->Option<Hit> {
        if self.nodes.is_empty() || !origin.finite() || !max.is_finite() || max<=0.0 { return None; }
        let dir=direction.normalized()?;
        let mut best=max; let mut result=None; let mut stack=vec![0];
        while let Some(i)=stack.pop() {
            let n=&self.nodes[i]; if !n.bounds.ray(origin,dir,best) { continue; }
            if let Some((a,b))=n.children { stack.extend([a,b]); continue; }
            for &t in &self.triangles[n.start..n.end] {
                let e1=t.b-t.a; let e2=t.c-t.a; let p=dir.cross(e2); let det=e1.dot(p);
                if det.abs()<1e-8 { continue; }
                let inv=1.0/det; let q=origin-t.a; let u=q.dot(p)*inv;
                if !(-1e-6..=1.000001).contains(&u) { continue; }
                let r=q.cross(e1); let v=dir.dot(r)*inv;
                if v< -1e-6 || u+v>1.000001 { continue; }
                let d=e2.dot(r)*inv;
                if d>=0.001 && d<=best {
                    let mut normal=t.cross().normalized()?;
                    if normal.dot(dir)>0.0 { normal = -normal; }
                    best=d; result=Some(Hit {distance:d,point:origin+dir*d,normal});
                }
            }
        }
        result
    }
    pub fn sphere_overlaps(&self,p:V3,r:f32)->bool {
        if !p.finite() || !r.is_finite() || r<0.0 { return true; }
        if self.nodes.is_empty() { return false; }
        let mut stack=vec![0];
        while let Some(i)=stack.pop() {
            let n=&self.nodes[i]; if !n.bounds.sphere(p,r) { continue; }
            if let Some((a,b))=n.children { stack.extend([a,b]); continue; }
            for &t in &self.triangles[n.start..n.end] {
                let d=t.closest(p)-p;
                if d.dot(d)<r*r { return true; }
            }
        }
        false
    }
    /// A conservative column clearance check for a standing/seated landing point.
    /// This is NOT Ratchet's character controller or a room-scale collision solver.
    pub fn landing_clear(&self,floor:V3,height:f32)->bool {
        let height=height.clamp(1.0,2.4);
        let mut y=0.32;
        while y<=height {
            if self.sphere_overlaps(floor+V3(0.0,y,0.0),0.25) { return false; }
            y+=0.2;
        }
        !self.sphere_overlaps(floor+V3(0.0,height,0.0),0.16)
    }
    pub fn teleport_destination(&self,origin:V3,direction:V3,height:f32)->Option<V3> {
        let hit=self.ray(origin,direction,12.0)?;
        if hit.normal.1<0.85 || hit.distance<0.4 { return None; }
        // Do not accept a ceiling's underside, even if triangle winding is inconsistent.
        if direction.1>=-0.05 { return None; }
        self.landing_clear(hit.point,height).then_some(hit.point)
    }
}

pub fn sphere_ray(origin:V3,direction:V3,centre:V3,radius:f32,max:f32)->Option<f32> {
    if !origin.finite() || !centre.finite() || !radius.is_finite() || radius<=0.0 || !max.is_finite() || max<=0.0 { return None; }
    let d=direction.normalized()?; let oc=origin-centre;
    let b=oc.dot(d); let c=oc.dot(oc)-radius*radius; let disc=b*b-c;
    if disc<0.0 { return None; }
    let root=disc.sqrt(); let near=-b-root; let far=-b+root;
    let t=if near>=0.0 {near} else {far};
    (t>=0.0 && t<=max).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn floor() -> Bvh { Bvh::new(vec![
        Triangle {a:V3(-10.0,0.0,-10.0),b:V3(10.0,0.0,-10.0),c:V3(10.0,0.0,10.0)},
        Triangle {a:V3(-10.0,0.0,-10.0),b:V3(10.0,0.0,10.0),c:V3(-10.0,0.0,10.0)}]) }
    fn close(a:V3,b:V3) { assert!((a-b).length()<1e-4,"{a:?} != {b:?}"); }
    #[test] fn coordinates_preserve_handedness_and_scale() {
        assert_eq!(game_to_metres([2.0,4.0,6.0],2.0),Ok(V3(1.0,3.0,-2.0)));
        let x=game_to_metres([1.0,0.0,0.0],1.0).unwrap();
        let y=game_to_metres([0.0,1.0,0.0],1.0).unwrap();
        let z=game_to_metres([0.0,0.0,1.0],1.0).unwrap();
        close(x.cross(y),z);
    }
    #[test] fn rejects_invalid_scale_and_positions() {
        for s in [0.0,-1.0,f32::NAN,f32::INFINITY] { assert!(game_to_metres([0.0;3],s).is_err()); }
        assert!(game_to_metres([f32::NAN,0.0,0.0],1.0).is_err());
    }
    #[test] fn snap_keeps_head_fixed() {
        let mut r=Rig {origin:V3(10.0,2.0,-3.0),yaw:0.7}; let head=V3(1.0,1.7,0.8);
        let before=r.world(head); r.snap(0.5,head); close(before,r.world(head));
    }
    #[test] fn teleport_preserves_physical_height() {
        let mut r=Rig {origin:V3::ZERO,yaw:1.3}; let head=V3(0.8,1.65,-0.7);
        r.teleport(V3(3.0,5.0,7.0),head); close(r.world(head),V3(3.0,6.65,7.0));
    }
    #[test] fn stereo_baseline_is_not_scaled_by_locomotion() {
        let r=Rig {origin:V3(4.0,2.0,3.0),yaw:2.0};
        assert!(((r.world(V3(-0.032,1.7,0.0))-r.world(V3(0.032,1.7,0.0))).length()-0.064).abs()<1e-6);
    }
    #[test] fn trigger_requires_release_at_startup() {
        let mut g=TriggerGate::default(); assert_eq!(g.update(Some(1.0)),Edge::None);
        assert_eq!(g.update(Some(0.0)),Edge::None); assert_eq!(g.update(Some(1.0)),Edge::Pressed);
        assert_eq!(g.update(Some(1.0)),Edge::None); assert_eq!(g.update(Some(0.5)),Edge::None);
        assert_eq!(g.update(Some(0.0)),Edge::Released);
    }
    #[test] fn losing_focus_cancels_instead_of_releasing() {
        let mut g=TriggerGate::default(); g.update(Some(0.0)); g.update(Some(1.0));
        assert_eq!(g.update(None),Edge::None); assert!(!g.held());
        assert_eq!(g.update(Some(1.0)),Edge::None);
    }
    #[test] fn snap_requires_neutral_between_turns() {
        let mut g=SnapGate::default(); assert_eq!(g.update(Some(1.0)),0.0);
        g.update(Some(0.0)); assert!(g.update(Some(1.0))<0.0);
        assert_eq!(g.update(Some(1.0)),0.0); g.update(Some(0.0)); assert!(g.update(Some(-1.0))>0.0);
    }
    #[test] fn floor_ray_and_parallel_miss() {
        let b=floor(); let h=b.ray(V3(0.0,2.0,0.0),-V3::UP,5.0).unwrap();
        close(h.point,V3::ZERO); close(h.normal,V3::UP);
        assert!(b.ray(V3(0.0,2.0,0.0),V3(1.0,0.0,0.0),5.0).is_none());
    }
    #[test] fn bvh_splits_and_returns_nearest_surface() {
        let mut t=Vec::new();
        for i in 0..30 { let y=i as f32; t.push(Triangle {a:V3(-2.0,y,-2.0),b:V3(2.0,y,-2.0),c:V3(0.0,y,2.0)}); }
        let b=Bvh::new(t); assert!(b.nodes.len()>1);
        assert!((b.ray(V3(0.0,40.0,0.0),-V3::UP,50.0).unwrap().distance-11.0).abs()<1e-5);
    }
    #[test] fn target_behind_wall_cannot_be_hit() {
        let wall=Bvh::new(vec![Triangle {a:V3(-3.0,-3.0,-2.0),b:V3(3.0,-3.0,-2.0),c:V3(0.0,3.0,-2.0)}]);
        let d=V3(0.0,0.0,-1.0); let stop=wall.ray(V3::ZERO,d,100.0).unwrap().distance;
        assert!(sphere_ray(V3::ZERO,d,V3(0.0,0.0,-5.0),0.5,stop).is_none());
        assert!(sphere_ray(V3::ZERO,d,V3(0.0,0.0,-1.0),0.2,stop).is_some());
    }
    #[test] fn sphere_inside_and_miss_cases() {
        assert_eq!(sphere_ray(V3::ZERO,V3(1.0,0.0,0.0),V3::ZERO,1.0,2.0),Some(1.0));
        assert!(sphere_ray(V3::ZERO,V3(1.0,0.0,0.0),V3(0.0,0.0,3.0),1.0,9.0).is_none());
    }
    #[test] fn closest_point_and_sphere_overlap() {
        let b=floor(); assert!(b.sphere_overlaps(V3(0.0,0.1,0.0),0.2));
        assert!(!b.sphere_overlaps(V3(0.0,0.5,0.0),0.2));
        assert!(b.landing_clear(V3::ZERO,1.7));
    }
    #[test] fn teleport_rejects_walls_and_accepts_floor() {
        let b=floor(); assert!(b.teleport_destination(V3(0.0,1.7,0.0),V3(0.0,-1.0,-1.0),1.7).is_some());
        assert!(b.teleport_destination(V3(0.0,1.7,0.0),V3(0.0,0.0,-1.0),1.7).is_none());
    }
    #[test] fn empty_and_degenerate_meshes_are_safe() {
        let b=Bvh::new(vec![Triangle {a:V3::ZERO,b:V3::ZERO,c:V3::ZERO}]);
        assert!(b.triangles().is_empty()); assert!(b.ray(V3::ZERO,V3::UP,10.0).is_none());
    }
}
