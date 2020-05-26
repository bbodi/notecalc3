# notecalc3

 wasm-pack build --debug --target no-modules; serve
 
 TODO:
[ ] Ha létrehozol egy változót, nem tudhatod melyik sorokban van alatta már
 hivatkozás rá.
 Rerender mindent vagy dependency check?
[ ] a konverzió müködjön megint in-nel to helyett
[ ] lineref után rögtön irni miért hibás?
[ ] max 4 tizedesjegy de jó lene ha állitható lenne kevessebbre

 
 
 Examples:
 
 - Lean gains
```
source: https://rippedbody.com/how-to-calculate-leangains-macros/

weight = 80 kg
height = 190 cm
age = 30

-- Step 1: Calculate your  (Basal Metabolic Rate) (BMR)
men BMR = 66 + (13.7 * weight/1kg) + (5 * height/1cm) - (6.8 * age)

'STEP 2. FIND YOUR TDEE BY ADJUSTING FOR ACTIVITY
Activity 
' Sedentary (little or no exercise) [BMR x 1.15]
' Mostly sedentary (office work), plus 3–6 days of weight lifting [BMR x 1.35]
' Lightly active, plus 3–6 days of weight lifting [BMR x 1.55]
' Highly active, plus 3–6 days of weight lifting [BMR x 1.75]
TDEE = (men BMR * 1.35)

'STEP 3. ADJUST CALORIE INTAKE BASED ON YOUR GOAL
Fat loss
    target weekly fat loss rate = 0.5%
    TDEE - ((weight/1kg) * target weekly fat loss rate * 1100)kcal
Muscle gain
    monthly rates of weight gain = 1%
    TDEE + (weight/1kg * monthly rates of weight gain * 330)kcal

Protein intake
    1.6 g/kg
    2.2 g/kg
    weight * &[35] to g
    weight * &[36] to g
Fat intake
    0.5g/kg or at least 30 %
    1g/kg minimum
    fat calory = 9
    &[32]
```
 